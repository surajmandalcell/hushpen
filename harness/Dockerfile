# Hushpen harness image (linux/arm64): build container and validation slot in one.
# Build: services.yaml `harness-image` (docker build --platform linux/arm64 -t hushpen-val:noble
#   -f harness/Dockerfile harness). The build context is harness/ only.
#
# Contents: GPUI and platform build deps, Mesa lavapipe (software Vulkan), the X11 test stack
# (Xvfb, openbox, xdotool, xclip, ImageMagick, xinput), PulseAudio, D-Bus with the portal and a
# StatusNotifier host, the paste targets, and the Rust toolchain named in rust-toolchain.toml.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8 LC_ALL=C.UTF-8

# Pipelining off and retries on: some mirrors and proxies serve a truncated .deb, which apt
# reports as "Hash Sum mismatch".
RUN printf 'Acquire::http::Pipeline-Depth "0";\nAcquire::Retries "5";\n' \
      >/etc/apt/apt.conf.d/99hushpen

# Inter is deliberately absent: the app embeds its own copy and the font check needs a system
# without one (`fc-list | grep -ci inter` prints 0).
# LLVM first and alone: in one big install its 26 MB package arrived corrupt on every attempt.
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends --fix-missing \
    ca-certificates curl git build-essential clang libclang-dev mold cmake pkg-config \
 && rm -rf /var/lib/apt/lists/*

RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends --fix-missing \
    libfontconfig-dev libfreetype-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
    libx11-dev libx11-xcb-dev libxcb1-dev libxcb-xkb-dev libxi-dev libxrandr-dev \
    libssl-dev libzstd-dev libasound2-dev libvulkan-dev libvulkan1 \
    libdbus-1-dev libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libxtst-dev \
    libxcb-xinput-dev libxcb-xtest0-dev libxcb-randr0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libpulse-dev \
 && rm -rf /var/lib/apt/lists/*

RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends --fix-missing \
    mesa-vulkan-drivers vulkan-tools libgl1-mesa-dri libegl1 libegl-mesa0 libgles2 libglx-mesa0 \
    mesa-utils \
    xvfb openbox wmctrl xdotool x11-utils x11-xserver-utils x11-apps xinput xclip xterm \
    imagemagick dbus dbus-x11 procps time \
    pulseaudio pulseaudio-utils libasound2-plugins alsa-utils \
    python3 python3-gi gir1.2-gtk-3.0 python3-xlib zenity at-spi2-core \
    xdg-desktop-portal xdg-desktop-portal-gtk \
    iproute2 strace jq file rpm squashfs-tools desktop-file-utils sqlite3 \
    fonts-dejavu-core fontconfig \
 && fc-cache -f >/dev/null \
 && rm -rf /var/lib/apt/lists/*

# Stock openbox has no maximize key. Alt+F10 -> ToggleMaximize lets the fixed-size check press
# the one key a user could use. Fail the build if the edit did not apply.
RUN sed -i 's#</keyboard>#  <keybind key="A-F10"><action name="ToggleMaximize"/></keybind>\n  </keyboard>#' \
      /etc/xdg/openbox/rc.xml \
 && grep -q 'key="A-F10"' /etc/xdg/openbox/rc.xml

# The toolchain is installed under the same name as rust-toolchain.toml (1.99.0) with the same
# components, so cargo in a checkout starts without a download. Keep both in step.
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal \
      --default-toolchain 1.99.0 -c rustfmt -c clippy \
 && rustc --version && cargo --version && cargo fmt --version && cargo clippy --version

# Registry and git caches, the target dir, and the repo are bind-mounted at run time.
WORKDIR /work
