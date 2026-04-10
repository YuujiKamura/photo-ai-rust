package master

import "embed"

// EmbeddedMaster contains the master CSV files (by_work_type/*.csv).
// These are copied here during the build process via `make all`.
//
//go:embed by_work_type/*.csv
var EmbeddedMaster embed.FS
