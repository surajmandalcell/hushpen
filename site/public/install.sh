#!/bin/sh
# Installs the latest Hushpen release on macOS (Apple silicon) or Linux (x64, arm64):
#   curl -fsSL https://hushpen.mandalsuraj.com/install.sh | sh
# HUSHPEN_INSTALL_DIR sets where the app goes. HUSHPEN_NO_OPEN=1 skips opening it.
set -eu

repo=surajmandalcell/hushpen

say() { printf '%s\n' "$*"; }
fail() { printf 'Hushpen install: %s\n' "$*" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || fail "curl is missing. Install curl, then run this again."

url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest") ||
  fail "could not reach GitHub."
tag=${url##*/}
version=${tag#v}
case $version in [0-9]*) ;; *) fail "could not find the latest release." ;; esac
base="https://github.com/$repo/releases/download/$tag"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

fetch() {
  say "Downloading $1"
  curl -fL --progress-bar -o "$tmp/$1" "$base/$1" || fail "could not download $1."
  curl -fsSL -o "$tmp/SHA256SUMS" "$base/SHA256SUMS" || fail "could not download SHA256SUMS."
  want=$(awk -v f="$1" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")
  [ -n "$want" ] || fail "$1 is not in SHA256SUMS."
  if command -v sha256sum >/dev/null 2>&1; then
    got=$(sha256sum "$tmp/$1" | awk '{ print $1 }')
  else
    got=$(shasum -a 256 "$tmp/$1" | awk '{ print $1 }')
  fi
  [ "$got" = "$want" ] || fail "$1 does not match its checksum. Nothing was installed."
}

case $(uname -s) in
Darwin)
  [ "$(uname -m)" = arm64 ] || fail "Hushpen needs a Mac with Apple silicon."
  file="Hushpen-$version-macos-arm64.zip"
  fetch "$file"
  ditto -x -k "$tmp/$file" "$tmp/app"
  dir=${HUSHPEN_INSTALL_DIR:-/Applications}
  if [ -z "${HUSHPEN_INSTALL_DIR:-}" ] && [ ! -w /Applications ]; then dir="$HOME/Applications"; fi
  mkdir -p "$dir"
  rm -rf "$dir/Hushpen.app"
  ditto "$tmp/app/Hushpen.app" "$dir/Hushpen.app"
  say "Installed Hushpen $version in $dir."
  if [ "${HUSHPEN_NO_OPEN:-}" != 1 ]; then open "$dir/Hushpen.app"; fi
  ;;
Linux)
  case $(uname -m) in
  x86_64 | amd64) arch=x64 ;;
  aarch64 | arm64) arch=arm64 ;;
  *) fail "Hushpen runs on x64 and arm64 Linux only." ;;
  esac
  file="Hushpen-$version-linux-$arch.AppImage"
  fetch "$file"
  dir=${HUSHPEN_INSTALL_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/hushpen}
  bin="$HOME/.local/bin"
  apps=${XDG_DATA_HOME:-$HOME/.local/share}/applications
  mkdir -p "$dir" "$bin" "$apps"
  chmod +x "$tmp/$file"
  mv -f "$tmp/$file" "$dir/Hushpen.AppImage"
  curl -fsSL -o "$dir/hushpen.png" "https://raw.githubusercontent.com/$repo/$tag/assets/linux-icons/256x256.png" ||
    say "Could not download the icon. The app still works."
  # AppImages mount with FUSE 2; without it the app unpacks itself at each start instead.
  extract=
  if ! { ldconfig -p 2>/dev/null | grep -q 'libfuse\.so\.2'; }; then extract='export APPIMAGE_EXTRACT_AND_RUN=1'; fi
  cat >"$bin/hushpen" <<LAUNCHER
#!/bin/sh
$extract
exec "$dir/Hushpen.AppImage" "\$@"
LAUNCHER
  chmod +x "$bin/hushpen"
  cat >"$apps/hushpen.desktop" <<DESKTOP
[Desktop Entry]
Name=Hushpen
Comment=Private dictation
Exec=$bin/hushpen
Icon=$dir/hushpen.png
Terminal=false
Type=Application
Categories=Utility;
StartupWMClass=hushpen
DESKTOP
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" 2>/dev/null || true
  say "Installed Hushpen $version. Open it from your app menu, or run: hushpen"
  case ":$PATH:" in *":$bin:"*) ;; *) say "Add $bin to your PATH to run hushpen from a terminal." ;; esac
  if [ "${HUSHPEN_NO_OPEN:-}" != 1 ] && [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    nohup "$bin/hushpen" >/dev/null 2>&1 &
  fi
  ;;
*)
  fail "Hushpen runs on macOS and Linux. Get other downloads at https://github.com/$repo/releases"
  ;;
esac
