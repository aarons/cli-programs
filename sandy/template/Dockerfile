FROM docker/sandbox-templates:claude-code

# Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/home/agent/.cargo/bin:$PATH"

# Node.js tools (npm already included)
RUN npm install -g pnpm

# Tauri dependencies (for Linux container)
RUN sudo apt-get update && sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    && sudo rm -rf /var/lib/apt/lists/*

# Additional Rust tools
RUN cargo install cargo-watch cargo-expand

# Java and Maven
RUN sudo apt-get update && sudo apt-get install -y \
    openjdk-17-jdk \
    maven \
    && sudo rm -rf /var/lib/apt/lists/* \
    && ARCH=$(dpkg --print-architecture) \
    && sudo ln -sf /usr/lib/jvm/java-17-openjdk-${ARCH} /usr/lib/jvm/java-17
ENV JAVA_HOME=/usr/lib/jvm/java-17

# CLI binaries from configured binary_dirs (e.g., ~/.local/bin)
COPY assets/bin/ /usr/local/bin/
