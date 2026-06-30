# Minecraft Asset Tools

This directory owns mclone's local Minecraft asset pack tooling.

Canonical commands:

```bash
pnpm assets:pack
pnpm assets:pack:check
pnpm assets:pack:write-lock
pnpm texture-lab:pack-overlay
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
/tmp/mclone-texture-lab/mclone-default-overlay.pbp
/tmp/mclone-texture-lab/mclone-default-overlay.pbp.json
```

Use it as a runtime overlay while local reference blockstates/models still fill
in the rest of the asset chain:

```bash
MCLONE_ASSET_OVERLAY_PACK=/tmp/mclone-texture-lab/mclone-default-overlay.pbp pnpm native:timedemo:smoke
```
