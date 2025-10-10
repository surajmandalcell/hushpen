# ASR Pro Makefile
# Provides convenient targets for building, testing, and running the project

.PHONY: help test.backend run.api dev.api clean docker.build docker.up docker.down build.linux build.mac build.win dev.linux dev.mac dev.win run run.linux run.mac run.win

FRONTENDS_MAKE := $(MAKE) -C frontends

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
	@echo ""
	@$(FRONTENDS_MAKE) help

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
	$(FRONTENDS_MAKE) build.linux

build.mac: ## Build the macOS SwiftUI application
	$(FRONTENDS_MAKE) build.mac

build.win: ## Build the Windows WPF application
	$(FRONTENDS_MAKE) build.win

dev.linux: ## Set up and run Linux GTK4 frontend in development mode
	$(FRONTENDS_MAKE) dev.linux

dev.mac: ## Set up and run macOS SwiftUI frontend in development mode
	$(FRONTENDS_MAKE) dev.mac

dev.win: ## Set up and run Windows WPF frontend in development mode
	$(FRONTENDS_MAKE) dev.win

run: ## Run the compiled Linux GTK4 application
	$(FRONTENDS_MAKE) run

run.linux: ## Run the compiled Linux GTK4 application
	$(FRONTENDS_MAKE) run.linux

run.mac: ## Run the compiled macOS SwiftUI application
	$(FRONTENDS_MAKE) run.mac

run.win: ## Run the compiled Windows WPF application
	$(FRONTENDS_MAKE) run.win
