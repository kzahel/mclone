ARG STEAMRT4_SDK="registry.gitlab.steamos.cloud/steamrt/steamrt4/sdk@sha256:584939ebd7d2f1eec719e771fdde4ae3bd469ee741c783abb7fe812ddaaf3ee4"
FROM ${STEAMRT4_SDK}

ARG RUST_VERSION="1.97.0"
ARG RUSTUP_VERSION="1.28.2"
ARG RUSTUP_INIT_SHA256="20a06e644b0d9bd2fbdbfd52d42540bdde820ea7df86e92e533c073da0cdd43c"

LABEL org.opencontainers.image.title="mclone SteamRT4 Rust builder"
LABEL org.opencontainers.image.description="Pinned Steam Linux Runtime 4 SDK builder for mclone"
LABEL org.opencontainers.image.source="https://gitlab.com/ValveSoftware/steam-runtime"
LABEL com.mclone.steamrt4.build="4.0.20260608.242786"
LABEL com.mclone.rust.version="${RUST_VERSION}"

RUN command -v cc \
    && command -v curl \
    && command -v file \
    && command -v git \
    && command -v pkg-config \
    && dpkg-query --show \
        build-essential \
        ca-certificates \
        curl \
        file \
        git \
        libasound2-dev:amd64 \
        libudev-dev:amd64 \
        pkgconf:amd64

ENV RUSTUP_HOME=/opt/rust/rustup
ENV PATH=/opt/rust/cargo/bin:${PATH}
RUN curl --fail --location --silent --show-error \
        --output /tmp/rustup-init \
        "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init" \
    && printf '%s  %s\n' "${RUSTUP_INIT_SHA256}" /tmp/rustup-init \
        | sha256sum --check --strict \
    && chmod 0755 /tmp/rustup-init \
    && CARGO_HOME=/opt/rust/cargo /tmp/rustup-init \
        --default-toolchain "${RUST_VERSION}" \
        --default-host x86_64-unknown-linux-gnu \
        --profile minimal \
        --no-modify-path \
        -y \
    && rm -f /tmp/rustup-init \
    && chmod -R a+rX /opt/rust \
    && rustc --version --verbose \
    && cargo --version

ENV CARGO_HOME=/cargo-home
ENV CARGO_TARGET_DIR=/workspace/native/target/steamrt4
ENV HOME=/tmp/mclone-builder-home
ENV RUSTUP_TOOLCHAIN=1.97.0-x86_64-unknown-linux-gnu
WORKDIR /workspace
