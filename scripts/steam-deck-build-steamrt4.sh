#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
DOCKERFILE="$REPO_ROOT/containers/steamrt4/Dockerfile"
DOCKER_CONTEXT="$REPO_ROOT/containers/steamrt4"
TARGET_DIR="$REPO_ROOT/native/target/steamrt4"
BINARY="$TARGET_DIR/release/mclone-native-client"
RECEIPT="$TARGET_DIR/build-receipt.json"
CARGO_CACHE=${MCLONE_STEAMRT4_CARGO_CACHE:-"${XDG_CACHE_HOME:-$HOME/.cache}/mclone/steamrt4/cargo"}
DOCKER_CONFIG_ROOT=${MCLONE_STEAMRT4_DOCKER_CONFIG:-"${XDG_CACHE_HOME:-$HOME/.cache}/mclone/steamrt4/docker"}

STEAMRT4_BUILD=4.0.20260608.242786
STEAMRT4_DIGEST=sha256:584939ebd7d2f1eec719e771fdde4ae3bd469ee741c783abb7fe812ddaaf3ee4
RUST_VERSION=1.97.0
IMAGE=${MCLONE_STEAMRT4_BUILDER_IMAGE:-"mclone-steamrt4-builder:${STEAMRT4_BUILD}-rust${RUST_VERSION}"}
RUSTFLAGS_VALUE=${RUSTFLAGS:-}

die()
{
    echo "steamrt4-build: $*" >&2
    exit 1
}

require_command()
{
    command -v "$1" >/dev/null 2>&1 ||
        die "required command not found: $1"
}

require_command docker
require_command git
require_command jq
require_command readelf
require_command sha256sum

if [[ $RUSTFLAGS_VALUE == *target-cpu=native* ]]; then
    die "refusing target-cpu=native for a Steam Deck artifact"
fi

mkdir -p "$DOCKER_CONFIG_ROOT"
if [[ ! -f $DOCKER_CONFIG_ROOT/config.json ]]; then
    jq -n '{auths: {}}' >"$DOCKER_CONFIG_ROOT/config.json"
fi

docker_cli()
{
    DOCKER_CONFIG="$DOCKER_CONFIG_ROOT" docker "$@"
}

if ! docker_cli info >/dev/null 2>&1; then
    if id -nG | tr ' ' '\n' | grep -qx docker; then
        die "Docker is unavailable even though this shell has the docker group"
    fi
    if getent group docker | cut -d: -f4 | tr ',' '\n' | grep -qx "$(id -un)"; then
        die "Docker access needs a fresh login; meanwhile run this command through: sg docker -c 'pnpm steamdeck:build:steamrt4'"
    fi
    die "Docker daemon access is unavailable for $(id -un)"
fi

mkdir -p "$TARGET_DIR" "$CARGO_CACHE"

echo "Building pinned SteamRT4 SDK image: $IMAGE"
docker_cli build \
    --pull \
    --file "$DOCKERFILE" \
    --tag "$IMAGE" \
    "$DOCKER_CONTEXT"

echo "Building mclone-native-client inside SteamRT4 SDK"
docker_cli run \
    --rm \
    --init \
    --user "$(id -u):$(id -g)" \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    --env CARGO_HOME=/cargo-home \
    --env CARGO_TARGET_DIR=/workspace/native/target/steamrt4 \
    --env HOME=/tmp/mclone-builder-home \
    --env RUSTFLAGS="$RUSTFLAGS_VALUE" \
    --env RUSTUP_HOME=/opt/rust/rustup \
    --env RUSTUP_TOOLCHAIN="${RUST_VERSION}-x86_64-unknown-linux-gnu" \
    --mount "type=bind,src=$REPO_ROOT,dst=/workspace,readonly" \
    --mount "type=bind,src=$TARGET_DIR,dst=/workspace/native/target/steamrt4" \
    --mount "type=bind,src=$CARGO_CACHE,dst=/cargo-home" \
    --workdir /workspace \
    "$IMAGE" \
    cargo build \
        --release \
        --locked \
        --manifest-path native/Cargo.toml \
        -p mclone-native-client \
        --bin mclone-native-client

test -x "$BINARY" || die "container build did not produce $BINARY"

image_id=$(docker_cli image inspect --format '{{.Id}}' "$IMAGE")
tool_versions=$(docker_cli run \
    --rm \
    --user "$(id -u):$(id -g)" \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    --env HOME=/tmp/mclone-builder-home \
    --env RUSTUP_HOME=/opt/rust/rustup \
    --env RUSTUP_TOOLCHAIN="${RUST_VERSION}-x86_64-unknown-linux-gnu" \
    "$IMAGE" \
    sh -c 'rustc --version --verbose; cargo --version')
runtime_ldd=$(docker_cli run \
    --rm \
    --user "$(id -u):$(id -g)" \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    --mount "type=bind,src=$BINARY,dst=/artifact/mclone-native-client,readonly" \
    "$IMAGE" \
    ldd /artifact/mclone-native-client)
if grep -q 'not found' <<<"$runtime_ldd"; then
    die "SteamRT4 SDK cannot resolve every artifact dependency: $runtime_ldd"
fi

commit=$(git -C "$REPO_ROOT" rev-parse HEAD)
if [[ -z $(git -C "$REPO_ROOT" status --porcelain --untracked-files=normal) ]]; then
    dirty=false
else
    dirty=true
fi
built_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
binary_sha=$(sha256sum "$BINARY" | cut -d' ' -f1)
lock_sha=$(sha256sum "$REPO_ROOT/native/Cargo.lock" | cut -d' ' -f1)
glibc_max=$(readelf --version-info "$BINARY" \
    | grep -oE 'GLIBC_[0-9]+(\.[0-9]+)*' \
    | sort -Vu \
    | tail -1)
needed=$(readelf --dynamic "$BINARY" \
    | sed -n 's/.*Shared library: \[\(.*\)\]/\1/p' \
    | jq -Rsc 'split("\n") | map(select(length > 0))')

jq -n \
    --arg schema "mclone-steamrt4-build-v1" \
    --arg builtAt "$built_at" \
    --arg steamrt4Build "$STEAMRT4_BUILD" \
    --arg steamrt4Digest "$STEAMRT4_DIGEST" \
    --arg builderImage "$IMAGE" \
    --arg builderImageId "$image_id" \
    --arg rustVersion "$RUST_VERSION" \
    --arg toolVersions "$tool_versions" \
    --arg commit "$commit" \
    --argjson dirty "$dirty" \
    --arg cargoLockSha256 "$lock_sha" \
    --arg target "x86_64-unknown-linux-gnu" \
    --arg rustflags "$RUSTFLAGS_VALUE" \
    --arg binarySha256 "$binary_sha" \
    --arg glibcMax "$glibc_max" \
    --argjson needed "$needed" \
    --arg runtimeLdd "$runtime_ldd" \
    '{
        schema: $schema,
        builtAt: $builtAt,
        steamrt4: {
            build: $steamrt4Build,
            manifestDigest: $steamrt4Digest
        },
        builder: {
            image: $builderImage,
            imageId: $builderImageId
        },
        rust: {
            version: $rustVersion,
            versions: $toolVersions
        },
        source: {
            commit: $commit,
            dirty: $dirty,
            cargoLockSha256: $cargoLockSha256
        },
        target: $target,
        rustflags: $rustflags,
        artifact: {
            path: "native/target/steamrt4/release/mclone-native-client",
            sha256: $binarySha256,
            glibcMax: $glibcMax,
            neededLibraries: $needed,
            sdkLdd: $runtimeLdd
        }
    }' >"$RECEIPT"

echo "SteamRT4 artifact: $BINARY"
echo "SteamRT4 receipt:  $RECEIPT"
jq '{
    steamrt4: .steamrt4,
    rust: .rust.version,
    target,
    artifact: {
        sha256: .artifact.sha256,
        glibcMax: .artifact.glibcMax,
        neededLibraries: .artifact.neededLibraries
    }
}' "$RECEIPT"
