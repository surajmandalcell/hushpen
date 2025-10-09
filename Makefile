# ASR Pro Makefile
# Provides convenient targets for building, testing, and running the project

.PHONY: help test.backend run.api dev.api clean dev.linux dev.mac dev.win run.linux run.mac run.win

help: ## Display available targets
	@echo "ASR Pro - Available targets:"
	@echo "  test.backend  - Run backend tests"
	@echo "  run.api       - Build/compile and run the API in production mode"
	@echo "  dev.api       - Run the API with hot reload capability"
	@echo "  clean         - Clean up build artifacts and temporary files"
	@echo ""
	@echo "Development targets:"
	@echo "  dev.linux     - Set up and run Linux GTK4 frontend in development mode"
	@echo "  dev.mac       - Set up and run macOS SwiftUI frontend in development mode"
	@echo "  dev.win       - Set up and run Windows WPF frontend in development mode"
	@echo ""
	@echo "Run targets:"
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

dev.linux: ## Set up and run Linux GTK4 frontend in development mode
	@echo "Setting up Linux GTK4 development environment..."
	@if ! command -v cargo &> /dev/null; then \
		echo "Error: Rust/Cargo is not installed. Please install Rust first."; \
		exit 1; \
	fi
	@if ! command -v meson &> /dev/null; then \
		echo "Error: Meson is not installed. Please install Meson first."; \
		exit 1; \
	fi
	@echo "Navigating to frontends/linux/..."
	cd frontends/linux && \
	if [ ! -d "builddir" ]; then \
		echo "Setting up Meson build directory..."; \
		meson setup builddir || { echo "Error: Meson setup failed"; exit 1; }; \
	fi && \
	echo "Compiling with Meson..." && \
	meson compile -C builddir || { echo "Error: Compilation failed"; exit 1; } && \
	echo "Starting Linux application in development mode..." && \
	./builddir/src/asrpro || { echo "Error: Failed to start application"; exit 1; }

dev.mac: ## Set up and run macOS SwiftUI frontend in development mode
	@echo "Setting up macOS SwiftUI development environment..."
	@if ! command -v swift &> /dev/null; then \
		echo "Error: Swift is not installed. Please install Xcode or Swift toolchain first."; \
		exit 1; \
	fi
	@echo "Navigating to frontends/mac/..."
	cd frontends/mac && \
	echo "Building with Swift..." && \
	swift build || { echo "Error: Swift build failed"; exit 1; } && \
	echo "Starting macOS application in development mode..." && \
	swift run || { echo "Error: Failed to start application"; exit 1; }

dev.win: ## Set up and run Windows WPF frontend in development mode
	@echo "Setting up Windows WPF development environment..."
	@if ! command -v dotnet &> /dev/null; then \
		echo "Error: .NET SDK is not installed. Please install .NET SDK first."; \
		exit 1; \
	fi
	@echo "Navigating to frontends/windows/..."
	cd frontends/windows && \
	echo "Restoring NuGet packages..." && \
	dotnet restore || { echo "Error: Package restore failed"; exit 1; } && \
	echo "Building with .NET..." && \
	dotnet build || { echo "Error: Build failed"; exit 1; } && \
	echo "Starting Windows application in development mode..." && \
	dotnet run --project ASRPro || { echo "Error: Failed to start application"; exit 1; }

run.linux: ## Run the compiled Linux GTK4 application
	@echo "Running Linux GTK4 application..."
	@echo "Navigating to frontends/linux/..."
	cd frontends/linux && \
	if [ ! -d "builddir" ]; then \
		echo "Application not built yet. Building first..."; \
		meson setup builddir || { echo "Error: Meson setup failed"; exit 1; }; \
		meson compile -C builddir || { echo "Error: Compilation failed"; exit 1; }; \
		echo "Build complete."; \
	fi && \
	if [ ! -f "builddir/src/asrpro" ]; then \
		echo "Application not built. Building now..."; \
		meson compile -C builddir || { echo "Error: Compilation failed"; exit 1; }; \
		echo "Build complete."; \
	fi && \
	echo "Starting Linux application..." && \
	./builddir/src/asrpro || { echo "Error: Failed to start application"; exit 1; }

run.mac: ## Run the compiled macOS SwiftUI application
	@echo "Running macOS SwiftUI application..."
	@echo "Navigating to frontends/mac/..."
	cd frontends/mac && \
	echo "Starting macOS application..." && \
	swift run || { echo "Error: Failed to start application"; exit 1; }

run.win: ## Run the compiled Windows WPF application
	@echo "Running Windows WPF application..."
	@echo "Navigating to frontends/windows/..."
	cd frontends/windows && \
	echo "Starting Windows application..." && \
	dotnet run --project ASRPro || { echo "Error: Failed to start application"; exit 1; }