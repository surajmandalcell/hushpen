.DEFAULT_GOAL := help

.PHONY: dev help

dev:
	npm run electron:dev

help:
	@printf "Usage:\n"
	@printf "  make dev   Start the Electron desktop app\n"
	@printf "  make help  Show this help\n"
