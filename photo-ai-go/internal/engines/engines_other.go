//go:build !windows

package engines

import "embed"

// EmbeddedEngines is a placeholder for non-Windows builds.
// The actual engine binaries are only embedded in Windows builds
// (see engines_windows.go).
var EmbeddedEngines embed.FS
