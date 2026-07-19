# Translation and Release Strategy

Mclone uses Minecraft Java 1.17.1 as a behavioral and visual reference while
building a shared Rust engine and original product content. The live engine
under [`../native/`](../native/) serves all five client targets; reference use
does not define a separate implementation track or a preferred platform.

This document owns the boundary between reference-backed parity development
and eventual public distribution. Runtime and host ownership live in
[`architecture.md`](architecture.md), durable crate and app ownership live in
[`native-engine-architecture.md`](native-engine-architecture.md), and reference
bootstrap details live in
[`reference-minecraft.md`](reference-minecraft.md).

## Current Reference Use

The local, gitignored Minecraft 1.17.1 decomp and extracted assets support:

- behavioral research before parity-sensitive implementation;
- oracle fixtures and deterministic output comparisons;
- development-only visual comparison and asset-pipeline bring-up;
- the reference-locked `overworld` generation profile.

Parity-sensitive implementations preserve Java primitive semantics explicitly,
including integer wrapping, floating-point behavior, truncation versus floor,
and random draw order. The Minecraft reference tree remains a development
input, not live engine code or distributable product content.

The `overworld` profile targets Minecraft Java 1.17.1 seed parity as an oracle
and regression surface. The original `mclone-overworld-v1` profile owns the
product's creative terrain, biome, surface, decoration, cave, geology,
landmark, and structure direction. See
[`topics/mclone-overworld-generation.md`](topics/mclone-overworld-generation.md)
and [`worldgen-status.md`](worldgen-status.md).

## Public Release Boundary

Public release is planned once Mclone can ship as an independently
distributable product. Release readiness includes:

- a complete first-party visual and audio asset set with verified provenance;
- sufficiently complete original Mclone world generation and gameplay;
- no packaged Minecraft jars, decompiled source, extracted assets, models,
  textures, sounds, structure data, or other reference payloads;
- review and, where necessary, independent replacement of implementation that
  was produced as a direct source translation;
- release-artifact audits proving that development-only reference inputs are
  absent.

The repository is internal and unreleased while those conditions remain open.
That is a current release state, not a personal/home-use product goal.

First-party asset-pack provenance and the remaining reference-free startup
boundary are tracked in
[`topics/asset-pack-profiles.md`](topics/asset-pack-profiles.md). Original world
generation status is tracked separately so completion of the reference-locked
`overworld` profile is never mistaken for completion of Mclone's product
generator.

## Independent Implementation Process

When release review requires source-derived logic to be replaced, use a
separated specification and implementation process:

1. A reference session studies the relevant behavior and writes a functional
   specification without reusable source expression.
2. A separate implementation session works from that specification and public
   factual interfaces, without access to the decomp or translated code.
3. Oracle fixtures and black-box measurements validate behavior without making
   the reference implementation the code under test.
4. Release review checks both implementation provenance and packaged content.

AI assistance does not remove the need for this separation. Project policy
treats code produced with source access as reference-derived until it has been
reviewed or replaced for distribution.

## Oracle Testing

Ground truth for parity tests is observed Minecraft behavior, not an earlier
Mclone translation.

- The Java oracle harness and pinned server/client artifacts generate focused
  behavioral measurements.
- Fixtures record block, biome, heightmap, protocol, rendering, or runtime facts
  needed by Rust tests.
- Tests compare Mclone output with those committed facts.
- Fixtures intended for release must contain factual measurements only and
  remain subject to the release-artifact review.

Bootstrap and fixture-generation commands live in
[`reference-minecraft.md`](reference-minecraft.md). Shared fixtures live under
[`../test/fixtures/`](../test/fixtures/).

## Component Policy

| Component | Development policy | Release direction |
|---|---|---|
| Minecraft 1.17.1 `overworld` profile | Reference-locked parity and oracle surface | Retain only if implementation and distribution review permit it; never substitute it for original Mclone world generation |
| `mclone-overworld-v1` | Original terrain and content over shared engine mechanisms | Complete as the product world-generation profile |
| Textures, models, sounds, and structure data | Local Minecraft content may be used for development comparison | Ship only first-party or otherwise redistributable content |
| Renderer, physics, UI, networking, persistence, and platform adapters | Original shared Rust implementation informed by behavioral references | Ship after ordinary provenance and release review |
| Oracle fixtures and tooling | Retained development infrastructure | Include only factual, review-approved artifacts needed by the released project |

The detailed implementation state changes frequently. Current facts belong in
the linked topic and status documents rather than a second checklist here.
