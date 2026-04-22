# Translation strategy

Two-phase approach. Optimize for "it works at home" now; worry about distribution later only if we want to.

## Phase 1 — direct translation (now)

- **Goal:** working terrain gen + voxel engine running on the LAN for personal use (my daughter's laptop).
- **Approach:** line-by-line translate the relevant parts of the 1.17.1 decomp into Rust (primary target, for the WASM/web story) or C++.
- **Tooling:** AI agents (Claude / Codex / etc.) with the decomp in context can do the bulk of the translation. We review each layer, fix idiomatic-Rust issues (borrow checker, no shared-mutable graphs, `Copy` vs `Clone`), stitch modules together.
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
- Run it headless with a pinned seed, let it generate a region, read `world/region/*.mca` directly (Anvil format — parsers exist in Rust `fastanvil`, Python `anvil-parser`, etc.).
- Dump block IDs, biome IDs, heightmaps at specific coordinates → JSON fixtures.
- Tests assert our port produces identical output.
- **Fixtures are factual measurements of behavior, not expression.** Safe to distribute even if the implementation isn't.
- Build the oracle harness once. Reuses across Phase 1 (validate direct translation works) and Phase 2 (validate clean-room is equivalent).

## What to translate vs. use as-is

| Component | Approach |
|---|---|
| `cubiomes` (biome IDs + structure positions) | MIT-licensed — direct port to WASM, no laundering needed |
| 1.17.1 terrain gen (carvers, surface rules, features) | Direct translation in Phase 1 |
| 1.18+ density functions (if we go that route later) | Direct translation from 1.18 decomp in Phase 1 |
| Textures, block models, structure NBT | Use Minecraft's for dev (`docs/assets-plan.md`); replace for Phase 2 release |
| Renderer, physics, UI, networking, chunk storage | Original work — no source to translate |

## AI agents and copyright

Running decomp through an AI to produce Rust still yields a derivative work — the tool doesn't change the legal status of the output. What matters is whether the AI *had access to the source* during generation. For Phase 1 that's fine (not distributing). For Phase 2, discipline about agent context is essential: source code lives in one session, specs live in another, and implementation happens in a third with only specs visible.

## Action items

Ordered dependency chain for Phase 1 terrain gen:

- [ ] Oracle harness: script that runs server jar with a pinned seed, dumps `region/*.mca` to JSON fixtures
- [ ] Pick translation target and stand up crate layout (leaning Rust)
- [ ] Translate PRNG (`LegacyRandomSource`, `WorldgenRandom`) — smallest, cleanest unit, easy to oracle-test against Java's output for a sequence of `nextInt` calls
- [ ] Translate noise (`PerlinNoise`, `ImprovedNoise`) — validate against MC's Perlin output at specific sample points
- [ ] Translate `NoiseSampler` + `NoiseBasedChunkGenerator` — produces block-level terrain
- [ ] Translate carvers (`CaveWorldCarver`, `CanyonWorldCarver`)
- [ ] Wire up cubiomes (WASM) for biome IDs
- [ ] Surface rules + feature placement
