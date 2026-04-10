package web

import "embed"

// StaticAssets contains the web UI files (index.html, etc.)
// These are copied here during the build process.
//
//go:embed *.html *.json ghostty/dist/*
var StaticAssets embed.FS
