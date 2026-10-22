FROM debian:bookworm-slim

# Avoid interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# ── System packages ───────────────────────────────────────────────────────────
# Base tools, clang/lld for cargo-xwin, and the egui window's build deps:
# libudev (gilrs, the controllers) and zstd (linked through pkg-config). The
# window loads Vulkan/OpenGL/Wayland/X11 with dlopen; Mesa's software Vulkan
# (lavapipe) lets its headless snapshot tests render without a GPU.
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
    perl \
    make \
    libudev-dev \
    libzstd-dev \
    mesa-vulkan-drivers \
    && rm -rf /var/lib/apt/lists/*

# ── Bun, only as `node` so actions/checkout runs without Node.js ─────────────
ENV BUN_INSTALL=/usr/local
RUN curl -fsSL https://bun.sh/install | bash \
    && ln -s /usr/local/bin/bun /usr/local/bin/node

# ── Rust (the version rust-toolchain.toml pins, so jobs do not download it) ──
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y \
        --default-toolchain 1.98.0 \
        --profile minimal \
        --component rustfmt,clippy \
        --no-modify-path \
    && rustup target add x86_64-pc-windows-msvc

# ── cargo-xwin (Windows cross-compiler) ──────────────────────────────────────
# xwin handles downloading the MSVC sysroot at build time; no Windows SDK needed
RUN cargo install cargo-xwin

# Sanity checks
RUN rustc --version && cargo --version && cargo xwin --version
