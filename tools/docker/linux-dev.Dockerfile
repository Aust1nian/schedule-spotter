# Dev-only Linux build/test image: Tauri prerequisites, WebKitGTK WebDriver,
# Xvfb, Rust and Node. Mirrors what ci.yml installs on ubuntu-latest.
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential curl wget file ca-certificates git pkg-config cmake clang libclang-dev \
      libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libssl-dev librsvg2-dev \
      libayatana-appindicator3-dev webkit2gtk-driver xvfb xauth dbus-x11 lsof python3 \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && apt-get install -y nodejs && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.98.0 \
    && chmod -R a+rwX /opt/cargo /opt/rustup
