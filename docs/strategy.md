# Original Product and Reference Strategy

Mclone is building an original voxel sandbox on a shared Rust engine. Minecraft
Java 1.17.1 remains an optional behavioral and visual comparison source, but
Minecraft parity is no longer a project target or the default implementation
method. The live engine under [`../native/`](../native/) serves all five client
targets.

This document owns the boundary between original product development,
reference research, and eventual public distribution. Runtime and host
ownership live in
[`architecture.md`](architecture.md), durable crate and app ownership live in
[`native-engine-architecture.md`](native-engine-architecture.md), and reference
bootstrap details live in
[`reference-minecraft.md`](reference-minecraft.md).

## Current Reference Use

The local, gitignored Minecraft 1.17.1 decomp and extracted assets may support:

- bounded behavioral or architectural comparison;
- maintenance of retained legacy behavior and focused oracle fixtures;
- development-only visual comparison and asset-pipeline bring-up;
- historical research into Minecraft's implementation.

The Minecraft reference tree remains an optional development input, not live
engine code, distributable product content, or the source of truth for new
Mclone behavior. Original Mclone features should be designed from product
requirements and shared engine contracts rather than translated from Java.

`mclone-overworld-v1` is the product new-world default and owns the creative
terrain, biome, surface, decoration, cave, geology, landmark, ecology, and
structure direction. The stored `overworld` profile remains a selectable
Java-1.17-shaped legacy development surface; its current fixtures guard
against accidental drift but do not make parity an active goal. See
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
generation status is tracked separately from the legacy `overworld` profile so
reference maintenance is never mistaken for product progress.

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

## Optional Oracle Testing

When a bounded legacy/reference task explicitly calls for a Minecraft oracle,
ground truth is observed Minecraft behavior, not an earlier Mclone
translation. Oracle testing is not a general acceptance requirement for new
Mclone features.

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
| Minecraft 1.17-shaped `overworld` profile | Legacy development/reference surface; internal and mutable | Maintain, change, or retire only for an explicit need; never substitute it for original Mclone world generation |
| `mclone-overworld-v1` | Original terrain and content over shared engine mechanisms; product default | Continue as the product world-generation profile |
| Textures, models, sounds, and structure data | Local Minecraft content may be used for development comparison | Ship only first-party or otherwise redistributable content |
| Renderer, physics, UI, networking, persistence, and platform adapters | Original shared Rust implementation informed by behavioral references | Ship after ordinary provenance and release review |
| Oracle fixtures and tooling | Retained development infrastructure | Include only factual, review-approved artifacts needed by the released project |

The detailed implementation state changes frequently. Current facts belong in
the linked topic and status documents rather than a second checklist here.
