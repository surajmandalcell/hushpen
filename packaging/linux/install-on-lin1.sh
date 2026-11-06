#!/usr/bin/env bash
# Run from the permanent lin1 checkout. No remote copying or system packages.
set -euo pipefail

die() { printf '%s\n' "$*" >&2; exit 1; }

# Return success for a first install or a newer major/minor series.
needs_install() {
  local candidate=$1 installed=$2 force=$3
  [[ $candidate =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] || return 2
  [[ -z $installed ]] && return 0
  [[ $installed =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] || return 2
  [[ $force == true ]] && return 0
  local cm cn cp im in ip
  IFS=. read -r cm cn cp <<<"$candidate"
  IFS=. read -r im in ip <<<"$installed"
  (( cm > im || (cm == im && cn > in) ))
}

if [[ ${1:-} == --self-test ]]; then
  for row in '2.3.3,,false,0' '2.4.0,2.3.3,false,0' '3.0.0,2.9.9,false,0' \
    '2.3.4,2.3.3,false,1' '2.3.3,2.3.3,false,1' '2.2.9,2.3.3,false,1' \
    '2.3.4,2.3.3,true,0' '2.3.3-rc.1,,false,2' '2.3.3,bad,false,2'; do
    IFS=, read -r candidate installed force expected <<<"$row"
    actual=0
    needs_install "$candidate" "$installed" "$force" || actual=$?
    [[ $actual == "$expected" ]] || die "cadence failed: $row (got $actual)"
  done
  printf 'Cadence checks passed.\n'
  exit 0
fi

force=false
case ${1:-} in '') ;; --force) force=true ;; *) die 'Usage: bash packaging/linux/install-on-lin1.sh [--force|--self-test]' ;; esac
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || die 'Run this installer on lin1 Linux x86_64.'
mountpoint -q /mnt/nvme1 || die '/mnt/nvme1 must be mounted.'
root=/mnt/nvme1/data/hushpen
repo=$(cd "$(dirname "$0")/../.." && pwd -P)
[[ $repo == "$root/repo" ]] || die "Use the permanent checkout at $root/repo."
cd "$repo"
version=$(awk -F '"' '/^version = / {print $2; exit}' Cargo.toml)
installed=''
if [[ -f $root/install/version && -x $root/install/Hushpen.AppImage ]]; then
  installed=$(cat "$root/install/version")
elif [[ -x $root/install/Hushpen.AppImage ]]; then
  line=$(APPIMAGE_EXTRACT_AND_RUN=1 timeout 60 "$root/install/Hushpen.AppImage" --version)
  installed=${line#hushpen }
  installed=${installed%% *}
fi
result=0
needs_install "$version" "$installed" "$force" || result=$?
[[ $result != 2 ]] || die 'Only stable numeric versions can be installed.'
if [[ $result == 1 ]]; then
  printf 'Keep installed %s; source %s needs no major/minor install.\n' "$installed" "$version"
  exit 0
fi

export CARGO_HOME="$root/cargo" RUSTUP_HOME="$root/rustup" CARGO_TARGET_DIR="$root/target"
export PATH="$CARGO_HOME/bin:$root/tools/usr/bin:$root/tools/bin:$PATH"
export CARGO_BUILD_JOBS=4
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$root/cache}" TMPDIR="${TMPDIR:-$root/tmp}"
mkdir -p "$XDG_CACHE_HOME" "$TMPDIR"
export HUSHPEN_BUILD_VERSION="$version" GGML_NATIVE=OFF APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1
case ${CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS:-false} in false|0) ;; *) die 'Release debug assertions must stay off.' ;; esac
export CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=false
for tool in cargo cargo-packager mksquashfs unsquashfs patchelf flock timeout pgrep desktop-file-validate update-desktop-database sha256sum; do
  command -v "$tool" >/dev/null || die "Missing tool: $tool"
done
if pgrep -x LeagueofLegends >/dev/null; then
  die 'The LeagueofLegends game is running on this host. Retry after it exits.'
fi
for pair in "$HOME/Downloads/Hushpen|$root/downloads" "$HOME/dev/personal/hushpen|$repo"; do
  link=${pair%%|*}; target=${pair#*|}
  if [[ -e $link || -L $link ]]; then
    [[ $(readlink -m "$link") == "$target" ]] || die "Existing path must be preserved: $link"
  fi
done
mkdir -p "$root/install" "$root/downloads" "$root/app-data" "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/Downloads" "$HOME/dev/personal"
mkdir -p "$CARGO_TARGET_DIR"
if [[ ! -e $repo/target && ! -L $repo/target ]]; then
  ln -s "$CARGO_TARGET_DIR" "$repo/target"
fi
exec 9>"$root/install/install.lock"
flock -n 9 || die 'Another Hushpen install is running.'
du -sh "$CARGO_TARGET_DIR" 2>/dev/null || true
dist="$root/downloads/$version"
mkdir -p "$dist"
timeout --kill-after=30s 90m cargo xtask package --target linux-x64 --formats appimage --out "$dist"
timeout --kill-after=30s 15m cargo xtask package-verify --target linux-x64 --formats appimage --dir "$dist"
(cd "$dist" && sha256sum --check SHA256SUMS)
artifact="$dist/Hushpen-$version-linux-x64.AppImage"
chmod +x "$artifact"
line=$(timeout 60 "$artifact" --version)
[[ $line == "hushpen $version"* ]] || die "Unexpected packaged version: $line"

pending="$root/install/Hushpen.AppImage.pending"
trap 'rm -f "$root/install/Hushpen.AppImage.pending"' EXIT
cp "$artifact" "$pending"
cmp "$artifact" "$pending"
mv -f "$pending" "$root/install/Hushpen.AppImage"
cp assets/linux-icons/256x256.png "$root/install/hushpen.png"
cat >"$HOME/.local/bin/hushpen" <<'LAUNCHER'
#!/bin/sh
export HUSHPEN_DATA_DIR=/mnt/nvme1/data/hushpen/app-data
export APPIMAGE_EXTRACT_AND_RUN=1
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-/mnt/nvme1/data/hushpen/cache}"
export TMPDIR="${TMPDIR:-/mnt/nvme1/data/hushpen/tmp}"
export PATH="/mnt/nvme1/data/hushpen/tools/usr/bin:$PATH"
export LD_LIBRARY_PATH="/mnt/nvme1/data/hushpen/tools/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export ESPEAK_DATA_PATH=/mnt/nvme1/data/hushpen/tools/usr/share/espeak-ng-data
exec /mnt/nvme1/data/hushpen/install/Hushpen.AppImage "$@"
LAUNCHER
chmod +x "$HOME/.local/bin/hushpen"
desktop="$HOME/.local/share/applications/io.github.surajmandalcell.hushpen.desktop"
cat >"$desktop" <<DESKTOP
[Desktop Entry]
Name=Hushpen
Comment=Private speech recognition and dictation
Exec=$HOME/.local/bin/hushpen
Icon=$root/install/hushpen.png
Terminal=false
Type=Application
Categories=Utility;
DESKTOP
desktop-file-validate "$desktop"
update-desktop-database "$HOME/.local/share/applications"
for pair in "$HOME/Downloads/Hushpen|$root/downloads" "$HOME/dev/personal/hushpen|$repo"; do
  link=${pair%%|*}; target=${pair#*|}
  if [[ -e $link || -L $link ]]; then
    [[ $(readlink -f "$link") == "$target" ]] || die "Existing path must be preserved: $link"
  else
    ln -s "$target" "$link"
  fi
done
printf '%s\n' "$version" >"$root/install/version.pending"
mv -f "$root/install/version.pending" "$root/install/version"
du -sh "$CARGO_TARGET_DIR"
printf 'Installed %s. Installer retained in %s. Restart the running app before UI checks.\n' "$line" "$dist"
