.PHONY: build clean test lint check help

help:
	@echo "Available targets:"
	@echo "  make build   - Build release binary"
	@echo "  make clean   - Remove build artifacts"
	@echo "  make test    - Run tests (single-threaded)"
	@echo "  make lint    - Run clippy and fmt check"
	@echo "  make check   - lint + test"
	@echo "  make help    - Show this message"

build:
	cargo build --release
	mkdir -p bin
	cp target/release/orchid bin/

clean:
	rm -rf bin/ target/

test:
	cargo test -- --test-threads=1

lint:
	cargo clippy -- -D warnings
	cargo fmt --check

check: lint test

.DEFAULT_GOAL := help
