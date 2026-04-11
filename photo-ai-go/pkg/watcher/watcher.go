// Package watcher provides a folder watching library with callback support.
//
// It monitors a directory for file system events (create, modify, delete, rename)
// and invokes registered callbacks. This is a Go port of the deps/folder-watcher
// Rust crate, using fsnotify instead of the notify crate.
//
// Usage:
//
//	w, err := watcher.New("/path/to/watch",
//		watcher.WithFilter("pdf", "txt"),
//		watcher.OnCreate(func(path string) {
//			fmt.Println("created:", path)
//		}),
//	)
//	if err != nil { ... }
//	if err := w.Start(); err != nil { ... }
//	defer w.Stop()
package watcher

import (
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"

	"github.com/fsnotify/fsnotify"
)

// Callback is the function signature for event handlers.
type Callback func(path string)

// Option configures a FolderWatcher.
type Option func(*FolderWatcher)

// WithFilter sets file extension filters. Only files with these extensions
// trigger callbacks. Extensions should be provided without the leading dot.
func WithFilter(extensions ...string) Option {
	return func(w *FolderWatcher) {
		for _, ext := range extensions {
			w.extensions[strings.ToLower(strings.TrimPrefix(ext, "."))] = struct{}{}
		}
	}
}

// WithRecursive sets whether to watch directories recursively.
// Default is true.
func WithRecursive(recursive bool) Option {
	return func(w *FolderWatcher) {
		w.recursive = recursive
	}
}

// OnCreate registers a callback for file creation events.
func OnCreate(cb Callback) Option {
	return func(w *FolderWatcher) { w.onCreate = cb }
}

// OnModify registers a callback for file modification events.
func OnModify(cb Callback) Option {
	return func(w *FolderWatcher) { w.onModify = cb }
}

// OnDelete registers a callback for file deletion events.
func OnDelete(cb Callback) Option {
	return func(w *FolderWatcher) { w.onDelete = cb }
}

// OnRename registers a callback for file rename events.
func OnRename(cb Callback) Option {
	return func(w *FolderWatcher) { w.onRename = cb }
}

// OnAny registers a callback for any file system event.
func OnAny(cb Callback) Option {
	return func(w *FolderWatcher) { w.onAny = cb }
}

// FolderWatcher monitors a directory for file system events.
type FolderWatcher struct {
	path       string
	extensions map[string]struct{}
	recursive  bool

	onCreate Callback
	onModify Callback
	onDelete Callback
	onRename Callback
	onAny    Callback

	fsw     *fsnotify.Watcher
	stop    chan struct{}
	running atomic.Bool
	mu      sync.Mutex
}

// New creates a new FolderWatcher for the specified path.
// Returns an error if the path does not exist or is not a directory.
func New(path string, opts ...Option) (*FolderWatcher, error) {
	if err := validatePath(path); err != nil {
		return nil, err
	}

	w := &FolderWatcher{
		path:       path,
		extensions: make(map[string]struct{}),
		recursive:  true,
	}
	for _, opt := range opts {
		opt(w)
	}
	return w, nil
}

// Path returns the watched directory path.
func (w *FolderWatcher) Path() string {
	return w.path
}

// IsRunning reports whether the watcher is active.
func (w *FolderWatcher) IsRunning() bool {
	return w.running.Load()
}

// Start begins watching the folder. Returns ErrAlreadyRunning if already started.
func (w *FolderWatcher) Start() error {
	w.mu.Lock()
	defer w.mu.Unlock()

	if w.running.Load() {
		return ErrAlreadyRunning
	}

	fsw, err := fsnotify.NewWatcher()
	if err != nil {
		return &WatcherError{Op: "start", Err: err}
	}

	if w.recursive {
		if err := addRecursive(fsw, w.path); err != nil {
			fsw.Close()
			return err
		}
	} else {
		if err := fsw.Add(w.path); err != nil {
			fsw.Close()
			return &WatcherError{Op: "add", Path: w.path, Err: err}
		}
	}

	w.fsw = fsw
	w.stop = make(chan struct{})
	w.running.Store(true)

	go w.loop()
	return nil
}

// Stop stops the watcher. Returns ErrNotRunning if not started.
func (w *FolderWatcher) Stop() error {
	w.mu.Lock()
	defer w.mu.Unlock()

	if !w.running.Load() {
		return ErrNotRunning
	}

	close(w.stop)
	err := w.fsw.Close()
	w.running.Store(false)
	if err != nil {
		return &WatcherError{Op: "stop", Err: err}
	}
	return nil
}

func (w *FolderWatcher) loop() {
	for {
		select {
		case <-w.stop:
			return
		case ev, ok := <-w.fsw.Events:
			if !ok {
				return
			}
			if !w.matchesFilter(ev.Name) {
				continue
			}
			w.dispatch(ev)
		case _, ok := <-w.fsw.Errors:
			if !ok {
				return
			}
			// Errors are silently dropped, matching the Rust implementation
			// which only logs via log::info.
		}
	}
}

func (w *FolderWatcher) matchesFilter(path string) bool {
	if len(w.extensions) == 0 {
		return true
	}
	ext := strings.TrimPrefix(filepath.Ext(path), ".")
	if ext == "" {
		return false
	}
	_, ok := w.extensions[strings.ToLower(ext)]
	return ok
}

func (w *FolderWatcher) dispatch(ev fsnotify.Event) {
	p := ev.Name

	switch {
	case ev.Op.Has(fsnotify.Create):
		if w.onCreate != nil {
			w.onCreate(p)
		}
	case ev.Op.Has(fsnotify.Write):
		if w.onModify != nil {
			w.onModify(p)
		}
	case ev.Op.Has(fsnotify.Remove):
		if w.onDelete != nil {
			w.onDelete(p)
		}
	case ev.Op.Has(fsnotify.Rename):
		if w.onRename != nil {
			w.onRename(p)
		}
	}

	if w.onAny != nil {
		w.onAny(p)
	}
}
