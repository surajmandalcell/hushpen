# Frontend build helpers for ASR Pro.

.PHONY: help build.linux build.mac build.win dev.linux dev.mac dev.win run run.linux run.mac run.win

CARGO ?= cargo
LINUX_PROFILE ?= dev
LINUX_TARGET ?=
MAC_CONFIGURATION ?= debug
WIN_CONFIGURATION ?= Debug

ifeq ($(LINUX_PROFILE),release)
  LINUX_PROFILE_DIR := release
  LINUX_CARGO_FLAGS := --release
else ifeq ($(LINUX_PROFILE),dev)
  LINUX_PROFILE_DIR := debug
  LINUX_CARGO_FLAGS :=
else
  LINUX_PROFILE_DIR := $(LINUX_PROFILE)
  LINUX_CARGO_FLAGS := --profile $(LINUX_PROFILE)
endif

ifeq ($(strip $(LINUX_TARGET)),)
  LINUX_TARGET_FLAG :=
  LINUX_OUTPUT_DIR := target/$(LINUX_PROFILE_DIR)
else
  LINUX_TARGET_FLAG := --target $(LINUX_TARGET)
  LINUX_OUTPUT_DIR := target/$(LINUX_TARGET)/$(LINUX_PROFILE_DIR)
endif

LINUX_BINARY := $(LINUX_OUTPUT_DIR)/asrpro-gtk4

help:
	@echo "Frontend targets:"
	@echo "  build.linux   - Build the Linux GTK4 application"
	@echo "  build.mac     - Build the macOS SwiftUI application"
	@echo "  build.win     - Build the Windows WPF application"
	@echo "  dev.linux     - Run Linux frontend in development mode"
	@echo "  dev.mac       - Run macOS frontend in development mode"
	@echo "  dev.win       - Run Windows frontend in development mode"
	@echo "  run           - Alias for run.linux"
	@echo "  run.linux     - Build and run the Linux GTK4 application"
	@echo "  run.mac       - Build and run the macOS SwiftUI application"
	@echo "  run.win       - Build and run the Windows WPF application"

build.linux:
	@echo "Building Linux GTK4 application..."
	@if ! command -v $(CARGO) >/dev/null 2>&1; then \
		echo "Error: Rust/Cargo is not installed. Please install Rust first."; \
		exit 1; \
	fi
	cd linux && \
	$(CARGO) build $(LINUX_CARGO_FLAGS) $(LINUX_TARGET_FLAG) || { echo "Error: Cargo build failed"; exit 1; }

build.mac:
	@echo "Building macOS SwiftUI application..."
	@if ! command -v swift >/dev/null 2>&1; then \
		echo "Error: Swift is not installed. Please install Xcode or the Swift toolchain first."; \
		exit 1; \
	fi
	cd mac && \
	swift build -c $(MAC_CONFIGURATION) || { echo "Error: Swift build failed"; exit 1; }

build.win:
	@echo "Building Windows WPF application..."
	@if ! command -v dotnet >/dev/null 2>&1; then \
		echo "Error: .NET SDK is not installed. Please install the .NET SDK first."; \
		exit 1; \
	fi
	cd windows && \
	dotnet build -c $(WIN_CONFIGURATION) || { echo "Error: Build failed"; exit 1; }

dev.linux:
	@echo "Running Linux GTK4 application in development mode..."
	$(MAKE) run.linux

dev.mac:
	@echo "Running macOS SwiftUI application in development mode..."
	$(MAKE) run.mac

dev.win:
	@echo "Restoring Windows dependencies..."
	cd windows && \
	dotnet restore || { echo "Error: Package restore failed"; exit 1; }
	@echo "Running Windows WPF application in development mode..."
	$(MAKE) run.win

run: run.linux

run.linux: build.linux
	@echo "Running Linux GTK4 application..."
	cd linux && \
	echo "Starting Linux application..." && \
	./$(LINUX_BINARY) || { echo "Error: Failed to start application"; exit 1; }

run.mac: build.mac
	@echo "Running macOS SwiftUI application..."
	cd mac && \
	swift run -c $(MAC_CONFIGURATION) --skip-build || { echo "Error: Failed to start application"; exit 1; }

run.win: build.win
	@echo "Running Windows WPF application..."
	cd windows && \
	dotnet run --project ASRPro -c $(WIN_CONFIGURATION) --no-build || { echo "Error: Failed to start application"; exit 1; }
