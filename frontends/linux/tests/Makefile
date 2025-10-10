# Makefile for running ASRPro GTK4 frontend tests

.PHONY: test test-unit test-integration test-ui test-e2e test-performance test-all test-coverage clean

# Default test target
test:
	cargo test

# Run unit tests
test-unit:
	cargo test --lib tests/unit

# Run integration tests
test-integration:
	cargo test --test integration

# Run UI component tests
test-ui:
	cargo test --test ui

# Run end-to-end tests
test-e2e:
	cargo test --test e2e

# Run performance tests
test-performance:
	cargo test --release --benches

# Run all tests
test-all: test-unit test-integration test-ui test-e2e

# Run tests with coverage
test-coverage:
	cargo tarpaulin --out Html --output-dir coverage/

# Run tests in release mode
test-release:
	cargo test --release

# Run tests with verbose output
test-verbose:
	cargo test -- --nocapture

# Run tests with specific filter
test-filter:
	@if [ -z "$(FILTER)" ]; then \
		echo "Usage: make test-filter FILTER=<pattern>"; \
		exit 1; \
	fi
	cargo test -- --exact $(FILTER)

# Clean test artifacts
clean:
	cargo clean
	rm -rf coverage/
	rm -rf target/llvm-cov-target/

# Run tests with specific features
test-features:
	@if [ -z "$(FEATURES)" ]; then \
		echo "Usage: make test-features FEATURES=<comma-separated-features>"; \
		exit 1; \
	fi
	cargo test --features "$(FEATURES)"

# Run tests for a specific module
test-module:
	@if [ -z "$(MODULE)" ]; then \
		echo "Usage: make test-module MODULE=<module-name>"; \
		exit 1; \
	fi
	cargo test --lib $(MODULE)

# Run tests with ignored tests
test-ignored:
	cargo test -- --ignored

# Run tests with specific package
test-package:
	@if [ -z "$(PACKAGE)" ]; then \
		echo "Usage: make test-package PACKAGE=<package-name>"; \
		exit 1; \
	fi
	cargo test -p $(PACKAGE)

# Run tests on a specific target
test-target:
	@if [ -z "$(TARGET)" ]; then \
		echo "Usage: make test-target TARGET=<target-triple>"; \
		exit 1; \
	fi
	cargo test --target $(TARGET)

# Run tests with custom configuration
test-config:
	@if [ -z "$(CONFIG)" ]; then \
		echo "Usage: make test-config CONFIG=<config-file>"; \
		exit 1; \
	fi
	cargo test --config $(CONFIG)

# Run tests with specific directory
test-dir:
	@if [ -z "$(DIR)" ]; then \
		echo "Usage: make test-dir DIR=<directory>"; \
		exit 1; \
	fi
	cargo test --test-dir $(DIR)

# Run tests with specific test file
test-file:
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make test-file FILE=<test-file>"; \
		exit 1; \
	fi
	cargo test --test $(FILE)

# Run tests with specific test name
test-name:
	@if [ -z "$(NAME)" ]; then \
		echo "Usage: make test-name NAME=<test-name>"; \
		exit 1; \
	fi
	cargo test -- --exact $(NAME)

# Help target
help:
	@echo "Available test targets:"
	@echo "  test              - Run all tests"
	@echo "  test-unit         - Run unit tests"
	@echo "  test-integration  - Run integration tests"
	@echo "  test-ui           - Run UI component tests"
	@echo "  test-e2e          - Run end-to-end tests"
	@echo "  test-performance  - Run performance tests"
	@echo "  test-all          - Run all test suites"
	@echo "  test-coverage     - Run tests with coverage report"
	@echo "  test-release      - Run tests in release mode"
	@echo "  test-verbose      - Run tests with verbose output"
	@echo "  test-filter       - Run tests with specific filter (FILTER=<pattern>)"
	@echo "  clean             - Clean test artifacts"
	@echo "  test-features     - Run tests with specific features (FEATURES=<features>)"
	@echo "  test-module       - Run tests for a specific module (MODULE=<module>)"
	@echo "  test-ignored      - Run tests including ignored ones"
	@echo "  test-package      - Run tests for a specific package (PACKAGE=<package>)"
	@echo "  test-target       - Run tests for a specific target (TARGET=<target>)"
	@echo "  test-config       - Run tests with custom config (CONFIG=<config>)"
	@echo "  test-dir          - Run tests in a specific directory (DIR=<dir>)"
	@echo "  test-file         - Run a specific test file (FILE=<file>)"
	@echo "  test-name         - Run a specific test by name (NAME=<name>)"
	@echo "  help              - Show this help message"