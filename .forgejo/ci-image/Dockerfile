FROM debian:bookworm-slim

# Avoid interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# ── System packages ───────────────────────────────────────────────────────────
# Base tools + clang/lld for cargo-xwin + Tauri Linux runtime deps
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    zip \
    python3 \
    clang \
    lld \
    llvm \
    unzip \
    pkg-config \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev \
    # .deb bundling
    dpkg \
    dpkg-dev \
    # AppImage bundling
    squashfs-tools \
    file \
    # RPM bundling
    rpm \
    && rm -rf /var/lib/apt/lists/*

# ── Bun (also symlinked as `node` so actions/checkout works without Node.js) ──
ENV BUN_INSTALL=/usr/local
RUN curl -fsSL https://bun.sh/install | bash \
    && ln -s /usr/local/bin/bun /usr/local/bin/node

# ── Rust (nightly, for edition 2024) ─────────────────────────────────────────
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y \
        --default-toolchain nightly \
        --profile minimal \
        --no-modify-path \
    && rustup target add x86_64-pc-windows-msvc

# ── cargo-xwin (Windows cross-compiler) ──────────────────────────────────────
# xwin handles downloading the MSVC sysroot at build time; no Windows SDK needed
RUN cargo install cargo-xwin

# ── @tauri-apps/cli (for `bun tauri signer sign`) ────────────────────────────
# Install globally so it's available without bun install in each job
RUN bun add -g @tauri-apps/cli

# Sanity checks
RUN rustc --version && cargo --version && bun --version && cargo xwin --version
