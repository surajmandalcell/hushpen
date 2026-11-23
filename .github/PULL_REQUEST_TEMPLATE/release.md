Release vX.Y.Z of Hushpen. Branch `release/vX.Y.Z`.
Major: breaking change or new paid tier. Minor: features. Patch: fixes only.

## In this PR

- [ ] `Cargo.toml` `[workspace.package] version` is X.Y.Z (update build X*1000000 + Y*1000 + Z)
- [ ] `CHANGELOG.md` has the X.Y.Z entry

## After the merge

- [ ] Tag the merge commit `vX.Y.Z` (or `vX.Y.Z-rc.N`) and push the tag
- [ ] Run the Release workflow with that tag: build, sign, and draft for macOS, Windows, Linux, Android
- [ ] Draft checked and published
- [ ] Homebrew cask `hushpen`: version and sha256 in `samhqapp/homebrew-tap`
- [ ] `samhq_backend/releases.json`: `hushpen.macos` version, build, and `known_versions`
- [ ] Download links and version on hushpen.samhq.app
- [ ] Linux test install on oci1 or oci2 (major or minor), per `docs/building.md`
- [ ] Installed copy updated
