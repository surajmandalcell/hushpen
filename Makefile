# ASR Pro Makefile
# Unified build and development commands

# Variables
PYTHON := python3
VENV := .venv
SCRIPTS := scripts
PROJECT := asrpro

# Detect OS for platform-specific commands
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
	OPEN_CMD := open
else ifeq ($(UNAME_S),Linux)
	OPEN_CMD := xdg-open
else
	OPEN_CMD := start
endif

# Default target
.DEFAULT_GOAL := help

# Help command
.PHONY: help
help: ## Show this help message
	@echo "ASR Pro - Make Commands"
	@echo "======================="
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-15s %s\n", $$1, $$2}'

# Setup and Installation
.PHONY: setup
setup: ## Setup development environment (creates venv, installs deps)
	@echo "Setting up development environment..."
	@$(PYTHON) -m venv $(VENV)
	@echo "Virtual environment created at $(VENV)"
	@$(VENV)/bin/pip install --upgrade pip
	@$(VENV)/bin/pip install -r requirements.txt
	@echo "✅ Setup complete! Activate with: source $(VENV)/bin/activate"

.PHONY: install
install: ## Install dependencies in current environment
	@echo "Installing dependencies..."
	@pip install -r requirements.txt
	@echo "✅ Dependencies installed"

.PHONY: install-dev
install-dev: install ## Install development dependencies
	@echo "Installing development dependencies..."
	@pip install black flake8 mypy pytest pytest-cov
	@echo "✅ Development dependencies installed"

.PHONY: install-macos
install-macos: ## Install macOS-specific dependencies
	@echo "Installing macOS-specific dependencies..."
	@if [ -f requirements-macos.txt ]; then \
		pip install -r requirements-macos.txt; \
	else \
		echo "requirements-macos.txt not found, skipping..."; \
	fi

# Running the application
.PHONY: run
run: ## Run the application
	@echo "Starting ASR Pro..."
	@$(PYTHON) -m $(PROJECT)

.PHONY: dev
dev: ## Run in development mode with hot-reload
	@echo "Starting ASR Pro in development mode..."
	@$(PYTHON) $(SCRIPTS)/run.py

.PHONY: server
server: ## Run only the API server
	@echo "Starting ASR Pro API server..."
	@$(PYTHON) -c "from $(PROJECT).server import ServerThread; from $(PROJECT).model_manager import ModelManager; import time; mm = ModelManager(); st = ServerThread(mm); st.start(); print('Server running on http://localhost:7341'); time.sleep(999999)"

# Building
.PHONY: build
build: ## Build standalone executable with Nuitka
	@echo "Building standalone executable..."
	@$(PYTHON) $(SCRIPTS)/build_nuitka.py

.PHONY: build-py2app
build-py2app: ## Build macOS .app bundle with py2app
	@echo "Building macOS app bundle..."
	@if [ -f setup.py ]; then \
		$(PYTHON) setup.py py2app; \
	else \
		echo "setup.py not found. Creating basic setup.py..."; \
		$(PYTHON) $(SCRIPTS)/create_setup.py; \
		$(PYTHON) setup.py py2app; \
	fi

# Testing
.PHONY: test
test: ## Run tests
	@echo "Running tests..."
	@if [ -d tests ]; then \
		$(PYTHON) -m pytest tests/ -v; \
	else \
		echo "No tests directory found. Run 'make test-init' to create basic tests."; \
	fi

.PHONY: test-init
test-init: ## Initialize test directory with basic tests
	@echo "Creating test directory and basic tests..."
	@mkdir -p tests
	@$(PYTHON) $(SCRIPTS)/init_tests.py
	@echo "✅ Test directory created"

.PHONY: test-coverage
test-coverage: ## Run tests with coverage
	@echo "Running tests with coverage..."
	@$(PYTHON) -m pytest tests/ --cov=$(PROJECT) --cov-report=html --cov-report=term

# Code Quality
.PHONY: format
format: ## Format code with black
	@echo "Formatting code..."
	@black $(PROJECT) tests scripts --line-length 100

.PHONY: lint
lint: ## Lint code with flake8
	@echo "Linting code..."
	@flake8 $(PROJECT) tests scripts --max-line-length 100 --ignore E203,W503

.PHONY: type-check
type-check: ## Type check with mypy
	@echo "Type checking..."
	@mypy $(PROJECT) --ignore-missing-imports

.PHONY: check
check: format lint type-check ## Run all code quality checks

# Cleaning
.PHONY: clean
clean: ## Clean build artifacts and cache
	@echo "Cleaning build artifacts..."
	@rm -rf build/ dist/ *.egg-info
	@rm -rf __pycache__ $(PROJECT)/__pycache__
	@find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
	@find . -type f -name "*.pyc" -delete
	@find . -type f -name "*.pyo" -delete
	@find . -type f -name ".coverage" -delete
	@rm -rf .pytest_cache .mypy_cache htmlcov
	@echo "✅ Cleaned build artifacts"

.PHONY: clean-models
clean-models: ## Clean downloaded model cache
	@echo "Cleaning model cache..."
	@rm -rf ~/.cache/huggingface
	@rm -rf ~/.cache/torch
	@echo "✅ Model cache cleaned"

.PHONY: clean-all
clean-all: clean clean-models ## Clean everything including venv
	@echo "Cleaning virtual environment..."
	@rm -rf $(VENV)
	@echo "✅ All artifacts cleaned"

# Utilities
.PHONY: tree
tree: ## Show project structure
	@tree -I '__pycache__|*.pyc|.git|.venv|build|dist|*.egg-info' -L 3

.PHONY: deps
deps: ## Show dependency tree
	@pip list --format=freeze

.PHONY: info
info: ## Show project information
	@echo "ASR Pro Project Information"
	@echo "==========================="
	@echo "Python: $$($(PYTHON) --version)"
	@echo "Platform: $$(uname -s)"
	@echo "Directory: $$(pwd)"
	@echo ""
	@echo "Installed Models:"
	@$(PYTHON) -c "from $(PROJECT).model_manager import MODEL_SPECS; print('  - ' + '\\n  - '.join(MODEL_SPECS.keys()))"

.PHONY: open-docs
open-docs: ## Open documentation in browser
	@echo "Opening documentation..."
	@$(OPEN_CMD) README.md

# Docker (if needed in future)
.PHONY: docker-build
docker-build: ## Build Docker image
	@echo "Building Docker image..."
	@docker build -t asrpro:latest .

.PHONY: docker-run
docker-run: ## Run in Docker container
	@echo "Running in Docker..."
	@docker run -it --rm -p 7341:7341 asrpro:latest

# Development shortcuts
.PHONY: r
r: run ## Shortcut for 'make run'

.PHONY: d
d: dev ## Shortcut for 'make dev'

.PHONY: t
t: test ## Shortcut for 'make test'

.PHONY: c
c: clean ## Shortcut for 'make clean'

.PHONY: f
f: format ## Shortcut for 'make format'