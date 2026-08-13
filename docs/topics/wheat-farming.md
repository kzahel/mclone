# Wheat Farming

Topic: `wheat-farming`

Status: **implementation active 2026-08-13 under Tactical
[`288`](../tactical/288-wheat-farming-foundation.md).**

## Purpose

This topic owns the first shared crop loop: player-created farmland, hydration,
random-tick growth, wheat harvest and renewal, durable field state, and the
ordinary-world sources used by any focused farming showcase. It should become
the reusable substrate for more crops, crop-aware pollination, farmstead
gardens, generated working fields, animal feed, and later food/crafting loops.

The governing direction is mechanics first. A field is not merely a patch of
crop textures placed by a fixture or generator; it is terrain the player can
transform and maintain through tools, water, seed, elapsed loaded-world time,
and harvest.

## Initial Contract

Tactical 288 owns the bounded first implementation. Its target is the useful
Minecraft Java 1.17.1 semantic shape: eight moisture states, eight visible
wheat ages, four-block irrigation reach, vanilla-shaped crop growth speed,
mature and immature loot, and loaded-chunk random ticking. The resulting block
states and inventory are ordinary persisted records.

The compact runtime terrain-state lane is still an interim identity map backed
by `u8` raw IDs. This slice can represent the sixteen required states, but it
leaves little remaining ID space. Do not compress future crop state or alias
distinct mechanics to evade that limit; widen or replace the interim lane when
the next content family needs it.

## Deliberate Gaps

- tool durability and general item components;
- rain hydration and farmland trampling;
- bone meal and enchantment-aware loot;
- grass/fern seed drops, crafting, bread, hunger, and cooking;
- bee-to-crop pollination effects;
- additional crops, crop genetics, seasons, pests, and soil fertility;
- villager farming and generated village/farmstead crop parcels; and
- unloaded-time catch-up.

These must consume the ordinary mechanics recorded here. They may not be
implemented only inside a showcase or authored settlement.

## Current Evidence

Implementation evidence pending.
