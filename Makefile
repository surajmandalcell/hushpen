.DEFAULT_GOAL := help

.PHONY: dev help

dev:
	npm run dev

help:
	@printf "Usage:\n"
	@printf "  make dev   Start the Vite dev server\n"
	@printf "  make help  Show this help\n"
