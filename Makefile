# ASR Pro Makefile
# Provides convenient targets for building, testing, and running the project

.PHONY: help test.backend run.api dev.api clean

help: ## Display available targets
	@echo "ASR Pro - Available targets:"
	@echo "  test.backend  - Run backend tests"
	@echo "  run.api       - Build/compile and run the API in production mode"
	@echo "  dev.api       - Run the API with hot reload capability"
	@echo "  clean         - Clean up build artifacts and temporary files"
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