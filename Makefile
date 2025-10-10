# ASR Pro Makefile
# Provides convenient targets for building, testing, and running the project

.PHONY: help test.backend run.api dev.api clean build.linux build.mac build.win dev.linux dev.mac dev.win run run.linux run.mac run.win

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

help: ## Display available targets
	@echo "ASR Pro - Available targets:"
	@echo "  test.backend  - Run backend tests"
	@echo "  run.api       - Build/compile and run the API in production mode"
	@echo "  dev.api       - Run the API with hot reload capability"
	@echo "  clean         - Clean up build artifacts and temporary files"
	@echo ""
	@echo "Build targets:"
	@echo "  build.linux   - Build the Linux GTK4 application"
	@echo "  build.mac     - Build the macOS SwiftUI application"
	@echo "  build.win     - Build the Windows WPF application"
	@echo ""
	@echo "Development targets:"
	@echo "  dev.linux     - Set up and run Linux GTK4 frontend in development mode"
	@echo "  dev.mac       - Set up and run macOS SwiftUI frontend in development mode"
	@echo "  dev.win       - Set up and run Windows WPF frontend in development mode"
	@echo ""
	@echo "Run targets:"
	@echo "  run           - Alias for run.linux"
	@echo "  run.linux     - Run the compiled Linux GTK4 application"
	@echo "  run.mac       - Run the compiled macOS SwiftUI application"
	@echo "  run.win       - Run the compiled Windows WPF application"
	@echo ""
	@echo "Docker targets:"
	@echo "  docker.build  - Build Docker containers"
	@echo "  docker.up     - Start Docker services"
	@echo "  docker.down   - Stop Docker services"

test.backend: ## Run backend tests
	@echo "Running backend tests..."
	cd sidecar && python -m pytest tests/ -v

run.api: ## Build/compile and run the API in production mode
	@echo "Starting ASR Pro Backend in production mode..."
	cd sidecar && \
	if [ ! -d "venv" ]; then \
		python3 -m venv venv; \
	fi; \
	source venv/bin/activate && \
	pip install -q -r requirements.txt && \
	python main.py

dev.api: ## Run the API with hot reload capability
	@echo "Starting ASR Pro Backend in development mode with hot reload..."
	cd sidecar && \
	if [ ! -d "venv" ]; then \
		python3 -m venv venv; \
	fi; \
	source venv/bin/activate && \
	pip install -q -r requirements.txt && \
	pip install -q watchfiles && \
	watchfiles --patterns "*.py" --recursive --command "python main.py" .

clean: ## Clean up build artifacts and temporary files
	@echo "Cleaning up build artifacts and temporary files..."
	find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name "*.pyc" -delete 2>/dev/null || true
	find . -type f -name "*.pyo" -delete 2>/dev/null || true
	find . -type d -name "*.egg-info" -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name ".coverage" -delete 2>/dev/null || true
	find . -type d -name ".pytest_cache" -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name ".mypy_cache" -exec rm -rf {} + 2>/dev/null || true
	rm -rf sidecar/venv 2>/dev/null || true
	@echo "Cleanup complete."

docker.build: ## Build Docker containers
	@echo "Building Docker containers..."
	docker-compose build

docker.up: ## Start Docker services
	@echo "Starting Docker services..."
	docker-compose up -d

docker.down: ## Stop Docker services
	@echo "Stopping Docker services..."
	docker-compose down

build.linux: ## Build the Linux GTK4 application
	@echo "Building Linux GTK4 application..."
	@if ! command -v $(CARGO) >/dev/null 2>&1; then \
		echo "Error: Rust/Cargo is not installed. Please install Rust first."; \
		exit 1; \
	fi
	cd frontends/linux && \
	$(CARGO) build $(LINUX_CARGO_FLAGS) $(LINUX_TARGET_FLAG) || { echo "Error: Cargo build failed"; exit 1; }

build.mac: ## Build the macOS SwiftUI application
	@echo "Building macOS SwiftUI application..."
	@if ! command -v swift >/dev/null 2>&1; then \
		echo "Error: Swift is not installed. Please install Xcode or the Swift toolchain first."; \
		exit 1; \
	fi
	cd frontends/mac && \
	swift build -c $(MAC_CONFIGURATION) || { echo "Error: Swift build failed"; exit 1; }

build.win: ## Build the Windows WPF application
	@echo "Building Windows WPF application..."
	@if ! command -v dotnet >/dev/null 2>&1; then \
		echo "Error: .NET SDK is not installed. Please install the .NET SDK first."; \
		exit 1; \
	fi
	cd frontends/windows && \
	dotnet build -c $(WIN_CONFIGURATION) || { echo "Error: Build failed"; exit 1; }

dev.linux: ## Set up and run Linux GTK4 frontend in development mode
	@echo "Setting up Linux GTK4 development environment..."
	@if ! command -v $(CARGO) >/dev/null 2>&1; then \
		echo "Error: Rust/Cargo is not installed. Please install Rust first."; \
		exit 1; \
	fi
	@echo "Running Linux application in development mode..."
	$(MAKE) run.linux

dev.mac: ## Set up and run macOS SwiftUI frontend in development mode
	@echo "Setting up macOS SwiftUI development environment..."
	@if ! command -v swift &> /dev/null; then \
		echo "Error: Swift is not installed. Please install Xcode or Swift toolchain first."; \
		exit 1; \
	fi
	@echo "Running macOS application in development mode..."
	$(MAKE) run.mac

dev.win: ## Set up and run Windows WPF frontend in development mode
	@echo "Setting up Windows WPF development environment..."
	@if ! command -v dotnet >/dev/null 2>&1; then \
		echo "Error: .NET SDK is not installed. Please install .NET SDK first."; \
		exit 1; \
	fi
	@echo "Restoring NuGet packages..."
	cd frontends/windows && \
	dotnet restore || { echo "Error: Package restore failed"; exit 1; }
	@echo "Running Windows application in development mode..."
	$(MAKE) run.win

run: run.linux

run.linux: build.linux ## Run the compiled Linux GTK4 application
	@echo "Running Linux GTK4 application..."
	cd frontends/linux && \
	echo "Starting Linux application..." && \
	./$(LINUX_BINARY) || { echo "Error: Failed to start application"; exit 1; }

run.mac: build.mac ## Run the compiled macOS SwiftUI application
	@echo "Running macOS SwiftUI application..."
	cd frontends/mac && \
	swift run -c $(MAC_CONFIGURATION) --skip-build || { echo "Error: Failed to start application"; exit 1; }

run.win: build.win ## Run the compiled Windows WPF application
	@echo "Running Windows WPF application..."
	cd frontends/windows && \
	dotnet run --project ASRPro -c $(WIN_CONFIGURATION) --no-build || { echo "Error: Failed to start application"; exit 1; }
