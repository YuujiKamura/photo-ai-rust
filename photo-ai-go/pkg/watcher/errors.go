package watcher

import (
	"errors"
	"fmt"
	"os"
)

var (
	// ErrAlreadyRunning is returned when Start is called on a running watcher.
	ErrAlreadyRunning = errors.New("watcher is already running")

	// ErrNotRunning is returned when Stop is called on a stopped watcher.
	ErrNotRunning = errors.New("watcher is not running")
)

// WatcherError wraps an underlying error with operation context.
type WatcherError struct {
	Op   string // operation that failed (e.g. "start", "add", "stop")
	Path string // related path, if any
	Err  error  // underlying error
}

func (e *WatcherError) Error() string {
	if e.Path != "" {
		return fmt.Sprintf("watcher %s %q: %v", e.Op, e.Path, e.Err)
	}
	return fmt.Sprintf("watcher %s: %v", e.Op, e.Err)
}

func (e *WatcherError) Unwrap() error { return e.Err }

// PathNotFoundError is returned when the watch target does not exist.
type PathNotFoundError struct{ Path string }

func (e *PathNotFoundError) Error() string {
	return fmt.Sprintf("path does not exist: %s", e.Path)
}

// NotADirectoryError is returned when the watch target is not a directory.
type NotADirectoryError struct{ Path string }

func (e *NotADirectoryError) Error() string {
	return fmt.Sprintf("path is not a directory: %s", e.Path)
}

// validatePath checks that path exists and is a directory.
func validatePath(path string) error {
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return &PathNotFoundError{Path: path}
		}
		return &WatcherError{Op: "stat", Path: path, Err: err}
	}
	if !info.IsDir() {
		return &NotADirectoryError{Path: path}
	}
	return nil
}
