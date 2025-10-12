.PHONY: help build.linux build.win dev.linux dev.win run run.linux run.win

CARGO ?= cargo
LINUX_PROFILE ?= dev
LINUX_TARGET ?=
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
	@echo "  build.win     - Build the Windows WPF application"
	@echo "  dev.linux     - Run Linux frontend in development mode"
	@echo "  dev.win       - Run Windows frontend in development mode"
	@echo "  run           - Alias for run.linux"
	@echo "  run.linux     - Build and run the Linux GTK4 application"
	@echo "  run.win       - Build and run the Windows WPF application"

build.linux:
	@echo "Building Linux GTK4 application..."
	@if ! command -v $(CARGO) >/dev/null 2>&1; then \
		echo "Error: Rust/Cargo is not installed. Please install Rust first."; \
		exit 1; \
	fi
	cd linux && \
	$(CARGO) build $(LINUX_CARGO_FLAGS) $(LINUX_TARGET_FLAG) || { echo "Error: Cargo build failed"; exit 1; }

build.win:
	@echo "Building Windows WPF application..."
	@where dotnet >nul 2>&1 || (echo "Error: .NET SDK is not installed. Please install the .NET SDK first." && exit /b 1)
	cd windows/ASRPro && dotnet build -c $(WIN_CONFIGURATION)

dev.linux:
	@echo "Running Linux GTK4 application in development mode..."
	$(MAKE) run.linux

dev.win:
	@echo "Killing any existing ASRPro instances..."
	@taskkill /F /IM ASRPro.exe 2>nul || echo No existing instances found
	@timeout /t 1 /nobreak >nul
	@echo "Restoring Windows dependencies..."
	cd windows/ASRPro && dotnet restore
	@echo "Running Windows WPF application in development mode..."
	cd windows/ASRPro && dotnet run -c $(WIN_CONFIGURATION)

run: run.linux

run.linux: build.linux
	@echo "Running Linux GTK4 application..."
	cd linux && \
	echo "Starting Linux application..." && \
	./$(LINUX_BINARY) || { echo "Error: Failed to start application"; exit 1; }

run.win: build.win
	@echo "Running Windows WPF application..."
	cd windows/ASRPro && \
	dotnet run -c $(WIN_CONFIGURATION) --no-build
