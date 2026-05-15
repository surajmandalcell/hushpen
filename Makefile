.DEFAULT_GOAL := help

.PHONY: dev help build\:mac build\:win build\:linux

dev:
	npm run electron:dev

build\:mac:
	@node scripts/build-electron.cjs mac $(ARGS)

build\:win:
	@node scripts/build-electron.cjs win --x64 $(ARGS)

build\:linux:
	@PATH="/opt/homebrew/opt/binutils/bin:/usr/local/opt/binutils/bin:$$PATH" node scripts/build-electron.cjs linux --x64 $(ARGS)

help:
	@printf "Usage:\n"
	@printf "  make dev          Start the Electron desktop app\n"
	@printf "  make build:mac    Build macOS release artifacts\n"
	@printf "  make build:win    Build Windows x64 release artifacts\n"
	@printf "  make build:linux  Build Linux release artifacts\n"
	@printf "  make help         Show this help\n"
	@printf "\n"
	@printf "Optional:\n"
	@printf "  make build:mac ARGS='--dir'  Pass extra args to electron-builder\n"
