.PHONY: build test clean

BINARY_NAME=photo-ai.exe
GO=C:/Program\ Files/Go/bin/go.exe
GIT_COMMIT=$(shell git rev-parse HEAD 2>/dev/null || echo unknown)

build:
	cd cmd/photo-ai && $(GO) build -ldflags "-X main.gitCommit=$(GIT_COMMIT)" -o ../../$(BINARY_NAME)

test:
	$(GO) test ./...

clean:
	rm -f $(BINARY_NAME)

# Build Rust engines first, then the Go main binary
all:
	cd .. && cargo build --release -p photo-ai-rust --bin photo-tag-engine
	cd .. && cargo build --release -p photo-engine --bin photo-analysis-engine
	cd .. && cargo build --release -p photo-ai-rust --bin photo-pdf-engine --features pdf-gen
	cd .. && cargo build --release -p photo-ai-rust --bin photo-excel-engine --features excel
	$(MAKE) build
