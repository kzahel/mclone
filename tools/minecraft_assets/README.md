# Minecraft Asset Tools

This directory owns mclone's local Minecraft asset pack tooling.

Canonical commands:

```bash
pnpm assets:pack
pnpm assets:pack:check
pnpm assets:pack:write-lock
pnpm texture-lab:pack-overlay
pnpm texture-lab:coverage
pnpm texture-lab:pack-authored
pnpm assets:pack:generated-fallback
pnpm assets:pack:diagnostic-missing
pnpm assets:pack:first-party
pnpm assets:validate:first-party
pnpm assets:pack:first-party:test
pnpm assets:stage:first-party
```

The checked freshness lock lives in:

```text
tools/minecraft_assets/locks/mclone-game-1.17.1.lock.json
```

Generated Mojang payloads remain ignored under `reference/minecraft-1.17.1/`.
The locked render pack is `extracted.zip`; local sound assets are separate
personal-validation files under `local-sounds/` and are staged to Android
outside the pack.

`tools/minecraft_assets/overlay_pack.py` builds first-party-only overlay packs
from an arbitrary root containing `assets/`. It does not read or fingerprint the
local Mojang extraction. The texture-lab wrapper writes:

```text
generated-assets/texture-lab/mclone-default-overlay.pbp
generated-assets/texture-lab/mclone-default-overlay.pbp.json
```

Set `MCLONE_TEXTURE_LAB_OUTPUT_ROOT` to move the texture-lab generated root to
another local directory.

Use it as a runtime overlay while local reference blockstates/models still fill
in the rest of the asset chain:

```bash
MCLONE_ASSET_OVERLAY_PACK=generated-assets/texture-lab/mclone-default-overlay.pbp pnpm native:timedemo:smoke
```

`pnpm texture-lab:coverage` compares the first-party overlay pack against the
native terrain atlas material set and writes:

```text
generated-assets/texture-lab/mclone-default-overlay-coverage.md
```

## Standalone First-Party Packs

`pnpm assets:pack:first-party` is the canonical build gate for the three
standalone first-party inputs:

```text
generated-assets/texture-lab/mclone-authored.pbp
generated-assets/texture-lab/mclone-generated-fallback.pbp
generated-assets/texture-lab/mclone-diagnostic-missing.pbp
```

The command first performs a clean texture-lab runtime export with
`--no-reference`, packages the accepted authored PNGs, far-LOD metadata, and
repo-owned figures, exports the shared Rust inventory, and builds the generated
sources. `generated-assets/` remains ignored.

The legacy-named generated fallback is now the provisional source. It contains
restrained deterministic color/noise textures for every canonical material,
generated actor/effect replacements, all required first-party figures, block
visual definitions, a provisional registry, coverage/provenance facts, and an
explicit suppressed-audio policy.

The separate diagnostic pack contains the conspicuous checker, magenta border,
and short-code PNGs. Missing codes start at four hexadecimal characters;
colliding prefixes extend deterministically and full-hash collisions fail the
build. The tiny checked-in font is `missing_font.v1.json`. Normal Mclone
Original selection does not include this pack.

Both PNG encoders use stored-DEFLATE streams, and both archives use fixed ZIP
timestamps and stored entries, so bytes do not vary with host zlib versions.
Manifests declare pack id, origin, roles, asset schema, and payload fingerprint.
The builder does not discover, read, fingerprint, or name
`reference/minecraft-1.17.1/`; isolated tests place a sentinel reference tree
beside the allowed inputs and prove it cannot enter any archive.

The combined build finishes by staging byte-identical copies and their sidecars
with a fingerprinted catalog under:

```text
generated-assets/first-party-stage/first-party-packs/
```

This directory is the canonical release/platform staging input. Desktop,
Android/XR, web, and offscreen packaging/discovery consume it; do not substitute
the partial `mclone-default-overlay.pbp` for either standalone artifact.

Run the strict runtime-facing audit with:

```bash
pnpm --silent assets:validate:first-party \
  > /tmp/mclone-first-party-provenance.json
```

The command opens only the staged authored and generated packs, prepares the
same terrain/actor/effect/LOD/audio inputs as the runtime, and emits the full
resolution ledger. It exits nonzero if any resolved entry has Minecraft or
unknown origin. Missing optional resources and explicitly suppressed audio stay
visible in the ledger but do not masquerade as resolved unknown content.
