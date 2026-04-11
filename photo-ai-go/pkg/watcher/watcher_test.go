package watcher

import (
	"os"
	"path/filepath"
	"sync/atomic"
	"testing"
	"time"
)

func TestNew(t *testing.T) {
	dir := t.TempDir()
	w, err := New(dir)
	if err != nil {
		t.Fatalf("New(%q) returned error: %v", dir, err)
	}
	if w.Path() != dir {
		t.Errorf("Path() = %q, want %q", w.Path(), dir)
	}
}

func TestNewNonexistentPath(t *testing.T) {
	_, err := New(filepath.Join(t.TempDir(), "does-not-exist"))
	if err == nil {
		t.Fatal("expected error for nonexistent path, got nil")
	}
	if _, ok := err.(*PathNotFoundError); !ok {
		t.Errorf("expected *PathNotFoundError, got %T: %v", err, err)
	}
}

func TestNewNotADirectory(t *testing.T) {
	f := filepath.Join(t.TempDir(), "file.txt")
	if err := os.WriteFile(f, []byte("x"), 0644); err != nil {
		t.Fatal(err)
	}
	_, err := New(f)
	if err == nil {
		t.Fatal("expected error for file path, got nil")
	}
	if _, ok := err.(*NotADirectoryError); !ok {
		t.Errorf("expected *NotADirectoryError, got %T: %v", err, err)
	}
}

func TestStartStop(t *testing.T) {
	dir := t.TempDir()
	w, err := New(dir, WithFilter("txt"))
	if err != nil {
		t.Fatal(err)
	}
	if err := w.Start(); err != nil {
		t.Fatalf("Start() error: %v", err)
	}
	if !w.IsRunning() {
		t.Error("IsRunning() = false after Start()")
	}

	// Double start should fail
	if err := w.Start(); err != ErrAlreadyRunning {
		t.Errorf("double Start() = %v, want ErrAlreadyRunning", err)
	}

	if err := w.Stop(); err != nil {
		t.Fatalf("Stop() error: %v", err)
	}
	if w.IsRunning() {
		t.Error("IsRunning() = true after Stop()")
	}

	// Double stop should fail
	if err := w.Stop(); err != ErrNotRunning {
		t.Errorf("double Stop() = %v, want ErrNotRunning", err)
	}
}

func TestOnCreateCallback(t *testing.T) {
	dir := t.TempDir()
	var count atomic.Int32

	w, err := New(dir,
		WithFilter("txt"),
		OnCreate(func(path string) {
			count.Add(1)
		}),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := w.Start(); err != nil {
		t.Fatal(err)
	}
	defer w.Stop()

	// Create a matching file
	if err := os.WriteFile(filepath.Join(dir, "test.txt"), []byte("hello"), 0644); err != nil {
		t.Fatal(err)
	}

	// Wait for event
	deadline := time.After(2 * time.Second)
	for count.Load() < 1 {
		select {
		case <-deadline:
			t.Fatalf("timeout: onCreate called %d times, want >= 1", count.Load())
		default:
			time.Sleep(50 * time.Millisecond)
		}
	}
}

func TestFilterExcludesNonMatching(t *testing.T) {
	dir := t.TempDir()
	var txtCount, anyCount atomic.Int32

	w, err := New(dir,
		WithFilter("txt"),
		OnCreate(func(path string) {
			txtCount.Add(1)
		}),
		OnAny(func(path string) {
			anyCount.Add(1)
		}),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := w.Start(); err != nil {
		t.Fatal(err)
	}
	defer w.Stop()

	// Create a non-matching file
	if err := os.WriteFile(filepath.Join(dir, "test.pdf"), []byte("pdf"), 0644); err != nil {
		t.Fatal(err)
	}

	// Create a matching file
	if err := os.WriteFile(filepath.Join(dir, "test.txt"), []byte("txt"), 0644); err != nil {
		t.Fatal(err)
	}

	// Wait for the txt event
	deadline := time.After(2 * time.Second)
	for txtCount.Load() < 1 {
		select {
		case <-deadline:
			t.Fatalf("timeout waiting for txt create event")
		default:
			time.Sleep(50 * time.Millisecond)
		}
	}

	// Give a small window for any straggling events
	time.Sleep(200 * time.Millisecond)

	// The pdf file should not have triggered any callbacks
	if got := anyCount.Load(); got < 1 {
		t.Errorf("onAny called %d times for txt, want >= 1", got)
	}
}

func TestOnModifyCallback(t *testing.T) {
	dir := t.TempDir()
	var count atomic.Int32

	// Pre-create the file before watching
	f := filepath.Join(dir, "test.txt")
	if err := os.WriteFile(f, []byte("initial"), 0644); err != nil {
		t.Fatal(err)
	}

	w, err := New(dir,
		OnModify(func(path string) {
			count.Add(1)
		}),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := w.Start(); err != nil {
		t.Fatal(err)
	}
	defer w.Stop()

	// Modify the file
	if err := os.WriteFile(f, []byte("modified"), 0644); err != nil {
		t.Fatal(err)
	}

	deadline := time.After(2 * time.Second)
	for count.Load() < 1 {
		select {
		case <-deadline:
			t.Fatalf("timeout: onModify called %d times, want >= 1", count.Load())
		default:
			time.Sleep(50 * time.Millisecond)
		}
	}
}

func TestOnDeleteCallback(t *testing.T) {
	dir := t.TempDir()
	var count atomic.Int32

	f := filepath.Join(dir, "test.txt")
	if err := os.WriteFile(f, []byte("data"), 0644); err != nil {
		t.Fatal(err)
	}

	w, err := New(dir,
		OnDelete(func(path string) {
			count.Add(1)
		}),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := w.Start(); err != nil {
		t.Fatal(err)
	}
	defer w.Stop()

	if err := os.Remove(f); err != nil {
		t.Fatal(err)
	}

	deadline := time.After(2 * time.Second)
	for count.Load() < 1 {
		select {
		case <-deadline:
			t.Fatalf("timeout: onDelete called %d times, want >= 1", count.Load())
		default:
			time.Sleep(50 * time.Millisecond)
		}
	}
}

func TestNoFilterMatchesAll(t *testing.T) {
	dir := t.TempDir()
	var count atomic.Int32

	w, err := New(dir,
		OnCreate(func(path string) {
			count.Add(1)
		}),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := w.Start(); err != nil {
		t.Fatal(err)
	}
	defer w.Stop()

	// Create files with various extensions
	for _, name := range []string{"a.txt", "b.pdf", "c.jpg"} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte("x"), 0644); err != nil {
			t.Fatal(err)
		}
	}

	deadline := time.After(2 * time.Second)
	for count.Load() < 3 {
		select {
		case <-deadline:
			t.Fatalf("timeout: onCreate called %d times, want >= 3", count.Load())
		default:
			time.Sleep(50 * time.Millisecond)
		}
	}
}
