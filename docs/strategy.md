# Translation strategy

Current approach: native-first Rust direct translation for parity-critical engine logic, with web/WASM kept alive as an early compatibility gate. The retired browser engine has been removed from the live tree; Git history is the archive for that implementation.

The durable native roadmap lives in [`native-rewrite-roadmap.md`](native-rewrite-roadmap.md). This document owns translation/oracle policy and the legal distinction between private direct translation and any future clean-room release.

For runtime boundaries that are intentionally not a 1:1 translation of Minecraft's host architecture, see [`architecture.md`](./architecture.md).

## Phase 1 — native direct translation (now)

- **Goal:** working terrain gen + voxel engine running on the LAN for personal use (my daughter's laptop).
- **Approach:** line-by-line translate the relevant parts of the 1.17.1 decomp into **Rust** crates under [`../native/`](../native/). Native desktop is the main development loop; the web target stays compiling/smoke-tested early so WASM/browser constraints remain visible. Android XR / Quest standalone is a future native target, so original Rust systems should avoid desktop-only shared contracts even while desktop is the active bring-up surface.
- **Tooling:** AI agents (Claude / Codex / etc.) with the decomp in context can do the bulk of the translation. We review each layer, preserve Java primitive semantics explicitly (`int` wrapping, `long` arithmetic, float/double behavior, truncation vs floor, JavaRandom draw counts), and stitch modules together behind oracle tests.
- **Legal status:** this code is a derivative work of Mojang's source. **Do not distribute.** `~/code/mclone` is a private GitHub repo; it stays private for this phase.
- **Why direct-first:** a literal translation is the shortest path to something that actually produces correct Minecraft-shaped terrain. Clean-room-first means debugging two unknowns at once ("is my reimplementation wrong, or does it just legitimately differ from MC?"). Direct-first gives us a known-working baseline and lets us oracle-test cleanly.

## Phase 2 — clean-room (only if we decide to distribute)

Not automatic. Triggered by an explicit decision to release as open source.

- Decomp + Phase-1 translation become *reference manuals*, not the codebase.
- Write a fresh implementation from functional specs (prose, not code snippets).
- AI agents can assist, but source must be walled off from their context. Two-agent pattern: one session reads the decomp and writes prose specs; a fresh session implements from specs.
- Validate against the same oracle fixtures as Phase 1 (see below). If both implementations match real MC, they're behaviorally equivalent; courts judge textual similarity of the *code*, not the output.

## Oracle testing

Ground truth for tests is **real Minecraft**, not our Phase-1 translation.

- `decompile-mc.sh --server` fetches the official server jar (same pipeline, different side).
- Run it headless with a pinned seed, let it generate a region, read `world/region/*.mca` directly (Anvil format — parsers exist in JS/TS `prismarine-nbt` + a thin region reader, Python `anvil-parser`, etc.).
- Dump block IDs, biome IDs, heightmaps at specific coordinates → JSON fixtures.
- Tests assert our port produces identical output.
- **Fixtures are factual measurements of behavior, not expression.** Safe to distribute even if the implementation isn't.
- Build the oracle harness once. Reuses across Phase 1 (validate direct translation works) and Phase 2 (validate clean-room is equivalent).

## What to translate vs. use as-is

| Component | Approach |
|---|---|
| 1.17.1 worldgen (PRNG, noise, biome source, terrain, carvers, surface, features, structure positions) | Direct translation from Java → Rust in Phase 1 |
| 1.18+ density functions (if we go that route later) | Direct translation from 1.18 decomp into Rust after the target is explicitly changed |
| Textures, block models, structure NBT | Use Minecraft's for dev (`docs/assets-plan.md`); replace for Phase 2 release |
| Renderer, physics, UI, networking, chunk storage | Original Rust/native work, shaped by [`native-rewrite-roadmap.md`](native-rewrite-roadmap.md) |

## AI agents and copyright

Running decomp through an AI to produce translated code still yields a derivative work — the tool and the output language don't change the legal status. What matters is whether the AI *had access to the source* during generation. For Phase 1 that's fine (not distributing). For Phase 2, discipline about agent context is essential: source code lives in one session, specs live in another, and implementation happens in a third with only specs visible.

## Action items

Ordered dependency chain for the native worldgen track:

- [x] Stand up Rust workspace and crate boundaries under [`../native/`](../native/)
- [x] Reuse existing oracle fixtures and Java reference tree as correctness inputs
- [x] Translate PRNG and JavaRandom-compatible worldgen seed helpers
- [x] Translate noise primitives, `NoiseSampler`, and terrain density fill
- [x] Translate `OverworldBiomeSource` enough for terrain/fixture parity
- [x] Translate surface and bedrock stage
- [x] Translate classic overworld AIR and LIQUID carvers
- [ ] Translate decorator composition and feature placement core
- [ ] Translate first block-mutating feature family, likely ores/underground features
- [ ] Reach native full decorated chunk parity against committed oracle fixtures
