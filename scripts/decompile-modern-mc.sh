#!/usr/bin/env bash
# Build the pinned modern Java side reference used for comparative worldgen
# research. This does not change Mclone's Minecraft Java 1.17.1 parity target.
#
# Usage:
#   ./scripts/decompile-modern-mc.sh [decompile-mc.sh flags]
#
# The default output is reference/minecraft-26.2/. Pass --out for a scratch
# tree. Run decompile-mc.sh directly without --only when a full decompile is
# genuinely required.

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
MODERN_REFERENCE_VERSION="26.2"

exec "$SCRIPT_DIR/decompile-mc.sh" \
    "$MODERN_REFERENCE_VERSION" \
    --client \
    --no-assets \
    --only net/minecraft/data/worldgen/biome/ \
    --only net/minecraft/data/worldgen/SurfaceRuleData \
    --only net/minecraft/data/worldgen/TerrainProvider \
    --only net/minecraft/world/level/biome/OverworldBiomeBuilder \
    --only net/minecraft/world/level/levelgen/Aquifer \
    --only net/minecraft/world/level/levelgen/Density \
    --only net/minecraft/world/level/levelgen/DensityFunction \
    --only net/minecraft/world/level/levelgen/DensityFunctions \
    --only net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator \
    --only net/minecraft/world/level/levelgen/NoiseChunk \
    --only net/minecraft/world/level/levelgen/NoiseRouter \
    --only net/minecraft/world/level/levelgen/NoiseRouterData \
    --only net/minecraft/world/level/levelgen/SurfaceRules \
    --only net/minecraft/world/level/levelgen/SurfaceSystem \
    "$@"
