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
	@echo "Copying engines from cargo target directory..."
	cp -f ../target/release/photo-tag-engine.exe ./internal/engines/ 2>/dev/null || echo "WARN: photo-tag-engine.exe not found in target/release"
	cp -f ../target/release/photo-analysis-engine.exe ./internal/engines/ 2>/dev/null || echo "WARN: photo-analysis-engine.exe not found in target/release"
	cp -f ../target/release/photo-pdf-engine.exe ./internal/engines/ 2>/dev/null || echo "WARN: photo-pdf-engine.exe not found in target/release"
	cp -f ../target/release/photo-excel-engine.exe ./internal/engines/ 2>/dev/null || echo "WARN: photo-excel-engine.exe not found in target/release"
	mkdir -p ./internal/web
	cp -f ../web/*.html ../web/*.json ./internal/web/ 2>/dev/null || echo "WARN: web assets not found"
	mkdir -p ./internal/master/by_work_type
	cp -f ../master/by_work_type/*.csv ./internal/master/by_work_type/ 2>/dev/null || echo "WARN: master CSVs not found"
	$(MAKE) build
