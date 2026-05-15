.DEFAULT_GOAL := help

.PHONY: dev help build\:mac build\:win build\:linux

dev:
	npm run electron:dev

build\:mac:
	@node scripts/build-electron.cjs mac $(ARGS)

build\:win:
	@node scripts/build-electron.cjs win $(ARGS)

build\:linux:
	@node scripts/build-electron.cjs linux $(ARGS)

help:
	@printf "Usage:\n"
	@printf "  make dev          Start the Electron desktop app\n"
	@printf "  make build:mac    Build macOS release artifacts\n"
	@printf "  make build:win    Build Windows release artifacts\n"
	@printf "  make build:linux  Build Linux release artifacts\n"
	@printf "  make help         Show this help\n"
	@printf "\n"
	@printf "Optional:\n"
	@printf "  make build:mac ARGS='--dir'  Pass extra args to electron-builder\n"
