#!/usr/bin/env bash
# Run from the permanent OCI checkout. No remote copying or system packages.
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

container() (
  run=$(mktemp -d "$root/cache/tmp/test-install.XXXXXXXX")
  trap 'if [[ -s $run/cid ]]; then docker --context default rm -f "$(cat "$run/cid")" >/dev/null 2>&1 || true; fi; rm -rf "$run"' EXIT
  trap 'exit 130' HUP INT TERM
  timeout --kill-after=30s 25m docker --context default run --rm --init \
    --cidfile "$run/cid" --name hushpen-test-install --cpus 2 --memory 5g \
    --log-driver local --log-opt max-size=10m --log-opt max-file=2 \
    -v "$repo:/src:ro" -v "$root:$root" \
    -v "$root/target/linux-arm64:/target" \
    -v "$root/cache/cargo:/opt/cargo" -v "$root/cache/rustup:/opt/rustup" \
    -v "$root/cache/tmp:/scratch" \
    -v "$HOME/.local/share/applications:/applications" -e HOME="$root/cache/home" \
    -e CARGO_HOME=/opt/cargo -e RUSTUP_HOME=/opt/rustup \
    -e CARGO_TARGET_DIR=/target -e TMPDIR=/scratch -e XDG_CACHE_HOME="$root/cache" \
    -e HUSHPEN_BUILD_VERSION -e GGML_NATIVE -e APPIMAGE_EXTRACT_AND_RUN -e NO_STRIP \
    -e CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=false -e CARGO_BUILD_JOBS=2 \
    -e PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    -w /src hushpen-val:noble "$@"
)

if [[ ${1:-} == --self-test ]]; then
  for row in '2.3.3,,false,0' '2.4.0,2.3.3,false,0' '3.0.0,2.9.9,false,0' \
    '2.3.4,2.3.3,false,1' '2.3.3,2.3.3,false,1' '2.2.9,2.3.3,false,1' \
    '2.3.4,2.3.3,true,0' '2.3.3-rc.1,,false,2' '2.3.3,bad,false,2'; do
    IFS=, read -r candidate installed force expected <<<"$row"
    actual=0
    needs_install "$candidate" "$installed" "$force" || actual=$?
    [[ $actual == "$expected" ]] || die "cadence failed: $row (got $actual)"
  done
  root=$(mktemp -d "${TMPDIR:?}/hushpen-installer.XXXXXXXX")
  trap 'rm -rf "$root"' EXIT
  mkdir -p "$root/cache/tmp"
  repo=$root/repo
  timeout() { shift 2; "$@"; return 124; }
  docker() {
    shift 2
    case $1 in
      run)
        while (( $# )); do
          if [[ $1 == --cidfile ]]; then printf 'test-container\n' >"$2"; break; fi
          shift
        done
        ;;
      rm)
        [[ $2 == -f && $3 == test-container ]] || die 'The cleanup selected a different container.'
        touch "$root/cleaned"
        ;;
      *) die 'Unexpected Docker command.' ;;
    esac
  }
  actual=0
  container true || actual=$?
  [[ $actual == 124 && -f $root/cleaned ]] || die 'Timed-out container cleanup failed.'
  [[ -z $(ls -A "$root/cache/tmp") ]] || die 'Container scratch remains after cleanup.'
  printf 'Cadence and timeout cleanup checks passed.\n'
  exit 0
fi

force=false
skip_build=false
for arg in "$@"; do
  case $arg in
    --force) force=true ;;
    --skip-build) skip_build=true ;;
    *) die 'Usage: bash packaging/linux/install-on-test-host.sh [--force] [--skip-build] | --self-test' ;;
  esac
done
[[ $(uname -s) == Linux && $(uname -m) == aarch64 ]] || die 'Run this installer on the selected OCI Linux arm64 host.'
mountpoint -q /mnt/sdb || die '/mnt/sdb must be mounted.'
root=/mnt/sdb/data/hushpen
host="$root/install/host"
if [[ -e $HOME/.local/share/hushpen-host || -L $HOME/.local/share/hushpen-host ]]; then
  [[ $(readlink -m "$HOME/.local/share/hushpen-host") == "$host" ]] || die 'Preserve the existing SSH host installation; move it to the data drive before retrying.'
fi
repo=$(cd "$(dirname "$0")/../.." && pwd -P)
[[ $repo == "$root/repo" ]] || die "Use the permanent checkout at $root/repo."
cd "$repo"
version=$(awk -F '"' '/^version = / {print $2; exit}' Cargo.toml)
installed=''
if [[ -f $root/install/version && -x $root/install/Hushpen.AppImage ]]; then
  installed=$(cat "$root/install/version")
elif [[ -x $root/install/Hushpen.AppImage ]]; then
  die 'The existing AppImage has no version receipt; preserve it and inspect the install first.'
fi
result=0
needs_install "$version" "$installed" "$force" || result=$?
[[ $result != 2 ]] || die 'Only stable numeric versions can be installed.'
if [[ $result == 1 ]]; then
  printf 'Keep installed %s; source %s needs no major/minor install.\n' "$installed" "$version"
  exit 0
fi

export CARGO_TARGET_DIR="$root/target/linux-arm64"
export HUSHPEN_BUILD_VERSION="$version" GGML_NATIVE=OFF APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1
case ${CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS:-false} in false|0) ;; *) die 'Release debug assertions must stay off.' ;; esac
export CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=false
for tool in docker flock timeout pgrep sha256sum tar; do
  command -v "$tool" >/dev/null || die "Missing tool: $tool"
done
docker --context default image inspect hushpen-val:noble >/dev/null || die 'Missing hushpen-val:noble image.'
if pgrep -x LeagueofLegends >/dev/null; then
  die 'The LeagueofLegends game is running on this host. Retry after it exits.'
fi
for pair in "$HOME/Downloads/Hushpen|$root/downloads" "$HOME/dev/personal/hushpen|$repo"; do
  link=${pair%%|*}; target=${pair#*|}
  if [[ -e $link || -L $link ]]; then
    [[ $(readlink -m "$link") == "$target" ]] || die "Existing path must be preserved: $link"
  fi
done
mkdir -p "$root/install" "$root/downloads" "$root/app-data" "$root/cache/tmp" "$root/cache/home" "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/Downloads" "$HOME/dev/personal"
mkdir -p "$CARGO_TARGET_DIR"
if [[ ! -e $repo/target && ! -L $repo/target ]]; then
  ln -s "$CARGO_TARGET_DIR" "$repo/target"
fi
exec 9>"$root/install/install.lock"
flock -n 9 || die 'Another Hushpen install is running.'
du -sh "$CARGO_TARGET_DIR" 2>/dev/null || true
dist="$root/downloads/$version"
mkdir -p "$dist"
if [[ $skip_build == false ]]; then
  container sh -ec 'command -v cargo-packager >/dev/null || cargo install --locked cargo-packager@0.11.8'
  container cargo xtask package --target linux-arm64 --formats appimage --out "$dist"
  container cargo xtask package-host --target linux-arm64 --out "$dist/host"
fi
container cargo xtask package-verify --target linux-arm64 --formats appimage --dir "$dist"
container chown -R "$(id -u):$(id -g)" "$dist"
(cd "$dist" && sha256sum --check --ignore-missing SHA256SUMS)
[[ -f $dist/host/Hushpen-$version-host-linux-arm64.tar.gz ]] || die 'The required host archive is missing.'
(cd "$dist/host" && awk -v name="Hushpen-$version-host-linux-arm64.tar.gz" \
  '$2 == name {print; found=1} END {exit !found}' SHA256SUMS | sha256sum --check)
artifact="$dist/Hushpen-$version-linux-arm64.AppImage"
chmod +x "$artifact"
line=$(container timeout 60 "$artifact" --version)
[[ $line == "hushpen $version"* ]] || die "Unexpected packaged version: $line"

pending="$root/install/Hushpen.AppImage.pending"
trap 'rm -f "$root/install/Hushpen.AppImage.pending"' EXIT
cp "$artifact" "$pending"
cmp "$artifact" "$pending"
mv -f "$pending" "$root/install/Hushpen.AppImage"
cp assets/linux-icons/256x256.png "$root/install/hushpen.png"
cat >"$HOME/.local/bin/hushpen" <<'LAUNCHER'
#!/bin/sh
export HUSHPEN_DATA_DIR=/mnt/sdb/data/hushpen/app-data
export APPIMAGE_EXTRACT_AND_RUN=1
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-/mnt/sdb/data/hushpen/cache}"
export TMPDIR="${TMPDIR:-/mnt/sdb/data/hushpen/cache/tmp}"
exec /mnt/sdb/data/hushpen/install/Hushpen.AppImage "$@"
LAUNCHER
chmod +x "$HOME/.local/bin/hushpen"
desktop="$HOME/.local/share/applications/hushpen.desktop"
cat >"$desktop" <<DESKTOP
[Desktop Entry]
Name=Hushpen
Comment=Private speech recognition and dictation
Exec=$HOME/.local/bin/hushpen
Icon=$root/install/hushpen.png
Terminal=false
Type=Application
Categories=Utility;
StartupWMClass=hushpen
DESKTOP
container desktop-file-validate /applications/hushpen.desktop
container update-desktop-database /applications
for pair in "$HOME/Downloads/Hushpen|$root/downloads" "$HOME/dev/personal/hushpen|$repo"; do
  link=${pair%%|*}; target=${pair#*|}
  if [[ -e $link || -L $link ]]; then
    [[ $(readlink -f "$link") == "$target" ]] || die "Existing path must be preserved: $link"
  else
    ln -s "$target" "$link"
  fi
done
mkdir -p "$host"
pending="$root/install/host.pending"
[[ ! -e $pending ]] || die "Inspect the unfinished host installation: $pending"
mkdir "$pending"
trap 'rm -f "$root/install/Hushpen.AppImage.pending"; rm -rf "$root/install/host.pending"' EXIT
tar -xzf "$dist/host/Hushpen-$version-host-linux-arm64.tar.gz" -C "$pending"
mv -f "$pending"/* "$host/"
rmdir "$pending"
[[ -e $HOME/.local/share/hushpen-host ]] || ln -s "$host" "$HOME/.local/share/hushpen-host"
[[ $("$host/hushpen-host" --version) == "hushpen-host $version" ]] || die 'Installed host version does not match.'
mkdir -p "$root/host-data" "$HOME/.config/hushpen-host" "$HOME/.config/systemd/user"
chmod 700 "$root/host-data" "$HOME/.config/hushpen-host"
if [[ ! -f $HOME/.config/hushpen-host/environment ]]; then
  printf 'HUSHPEN_DATA_DIR=%s\n' "$root/host-data" >"$HOME/.config/hushpen-host/environment"
  chmod 600 "$HOME/.config/hushpen-host/environment"
fi
install -m 644 packaging/hushpen-host.service "$HOME/.config/systemd/user/hushpen-host.service"
printf '%s\n' "$version" >"$root/install/version.pending"
mv -f "$root/install/version.pending" "$root/install/version"
du -sh "$CARGO_TARGET_DIR"
printf 'Installed %s and hushpen-host. Installers retained in %s.\n' "$line" "$dist"
printf 'Headless host: desktop startup, tray, global keys, microphone, and paste checks are skipped. Use the Xvfb harness for UI checks.\n'
