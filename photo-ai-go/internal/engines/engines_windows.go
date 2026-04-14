package engines

import "embed"

// EmbeddedEngines contains the Rust engine binaries (*.exe).
// These are copied here during the build process via `make all`.
// The .exe files are gitignored; they must exist at build time
// (CI downloads them, local dev runs `make all` first).
//
//go:embed *.exe
var EmbeddedEngines embed.FS
