package watcher

import (
	"os"
	"path/filepath"

	"github.com/fsnotify/fsnotify"
)

// addRecursive walks the directory tree rooted at path and adds each
// subdirectory to the fsnotify watcher.
func addRecursive(fsw *fsnotify.Watcher, root string) error {
	return filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return nil // skip inaccessible dirs
		}
		if d.IsDir() {
			if err := fsw.Add(path); err != nil {
				return &WatcherError{Op: "add", Path: path, Err: err}
			}
		}
		return nil
	})
}
