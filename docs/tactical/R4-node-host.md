# R4: Headless Node Host

This slice follows `R0` through `R3` in the runtime/host arc from [`README.md`](README.md). The authoritative host boundary already exists, browser singleplayer already runs through a worker, chunk meshing already lives behind a client worker boundary, and persistence already sits behind engine-native storage contracts. The next requirement is proving that the same authoritative runtime core also runs outside the browser.

## Scope

Add:

- a file-backed `WorldStorage` adapter behind the existing `WorldStorage` / `ChunkStorage` contracts
- a shared generated-world host factory reused by both the browser worker and the Node host bootstrap
- a headless Node bootstrap that opens a world, applies chunk-view requests, and reports a JSON summary
- a CLI/config entry point for the Node bootstrap
- integration coverage for both the direct Node runner and a spawned Node CLI process

Do not add:

- remote browser-client transport
- WebRTC or WebSocket work
- worker-thread pools for the Node host
- a long-term optimized on-disk chunk format
- gameplay save data beyond the current authoritative chunk snapshots and save metadata

## Reference shape vs divergence

Minecraft Java ships with a dedicated server process and an Anvil-backed save format, but that architecture is not something we should transliterate class-for-class here.

The relevant rule from [`../../AGENTS.md`](../../AGENTS.md) is:

1. simulation/content parity remains the default
2. runtime orchestration may diverge for browser / worker / Node constraints
3. those divergences must preserve a clear path for future parity work

This slice stays within that rule. The authoritative world host is still the same TypeScript simulation/runtime core. The divergence is only in the outer runtime shell:

- browser worker host uses IndexedDB
- Node host uses filesystem-backed storage
- both boot through the same generated-world host factory and the same host/client message shapes

## Landed shape

### File-backed storage adapter

Added `FileWorldStorage` under `src/runtime/storage/`, which:

- persists `WorldSaveMetadata` as JSON
- stores per-chunk authoritative `ChunkSnapshot` records as JSON files
- records adapter-local load/save/evict timestamps just like the browser adapter
- resets a save directory if its persisted metadata is incompatible with the requested authoritative world shape

This is intentionally a simple proof adapter, not the final dedicated-server storage format.

### Shared host bootstrap

Added `createGeneratedWorldHostForRequest(...)`, which now owns the generated-block registration and preset-specific smoke mutation hook. That keeps the browser worker and the Node host bootstrap aligned on the same authoritative host construction path.

### Node bootstrap and CLI

Added `src/runtime/node/headless-generated-world-host.ts`, which:

- loads config from CLI flags and/or a JSON config file
- boots the authoritative generated-world host with `FileWorldStorage`
- opens the requested world
- applies one or more chunk-view requests
- prints a JSON summary of the opened world and view results

The repo now exposes that path through:

```bash
pnpm host:node -- --save-root /tmp/mclone-node-worlds --seed 12345
```

The current CLI is intentionally local and one-shot. It proves the host runtime and storage path in Node before `R5` adds remote browser-client transport.

### Node runtime shim

The Node entry currently runs through a narrow loader shim in `scripts/node-ts-loader.mjs` plus Node’s `--experimental-transform-types` mode.

That divergence exists for one reason only: the repo’s TypeScript source currently uses Vite/Vitest-style extensionless TS imports, and some shared files still rely on TS syntax such as `namespace` declarations that `strip-only` execution cannot run. The shim keeps that concern scoped to the Node bootstrap path instead of forcing a repo-wide import-style churn in this slice.

## Why this shape

This cut proves the dedicated-host direction without redesigning the runtime boundary again:

- authoritative world ownership still lives behind `WorldHost`
- persistence still lives behind `WorldStorage`
- browser and Node differ only in the outer adapter/bootstrap layer
- remote multiplayer remains a transport problem for `R5`, not a reason to fork the simulation/runtime core

## Validation

- `pnpm typecheck`
- `pnpm test`
- `pnpm test:browser`
- inspected `/tmp/mclone-browser-smoke.png`

## Next step

`R5`: add a remote browser-client transport to the dedicated host so browser clients can consume the same authoritative chunk/state stream over a real network boundary.
