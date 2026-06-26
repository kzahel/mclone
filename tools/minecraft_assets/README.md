# Minecraft Asset Tools

This directory owns mclone's local Minecraft asset pack tooling.

Canonical commands:

```bash
pnpm assets:pack
pnpm assets:pack:check
pnpm assets:pack:write-lock
```

The checked freshness lock lives in:

```text
tools/minecraft_assets/locks/mclone-game-1.17.1.lock.json
```

Generated Mojang payloads remain ignored under `reference/minecraft-1.17.1/`.
The locked render pack is `extracted.zip`; local sound assets are separate
personal-validation files under `local-sounds/` and are staged to Android
outside the pack.
