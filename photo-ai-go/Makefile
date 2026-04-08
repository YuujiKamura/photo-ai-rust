.PHONY: build test clean

BINARY_NAME=photo-ai.exe
RUST_TARGET=../target/release

build:
	cd cmd/photo-ai && go build -o ../../$(BINARY_NAME)

test:
	go test ./...

clean:
	rm -f $(BINARY_NAME)
	rm -f photo-ai.exe

# Build Rust engines first, then Go CLI
all:
	cd .. && cargo build --release --bins
	$(MAKE) build
