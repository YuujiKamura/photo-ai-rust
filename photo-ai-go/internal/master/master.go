package master

import "embed"

// EmbeddedMaster contains the default master bundle that ships with the app.
// These files are seed data only; runtime should prefer user-writable copies.
//
//go:embed manifest.json by_work_type/*.csv
var EmbeddedMaster embed.FS
