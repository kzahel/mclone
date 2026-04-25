# Movement3 - Paused

Status: **paused / superseded**.

This file is kept only so existing links do not imply a missing document. Do not implement this tactical as the next movement slice.

## Why It Is Paused

The previous `Movement3` direction treated the next problem as "where does prediction run?" That is now too narrow. The vanilla source review in [`../minecraft-client-replica-research.md`](../minecraft-client-replica-research.md) shows that the client needs a broader replica model before movement prediction can be designed correctly:

- an integrated server / authoritative host boundary even for singleplayer
- a client world that owns visible chunks, block/fluid state, block entities, entities, and light/render facts
- a client runtime that hydrates that client world from the same protocol in singleplayer and multiplayer
- prediction and interpolation services that read from the client world instead of host internals
- a presentation/UI thread that receives compact presentation state and does not own raw world or collision data

The active tactical direction is now the Client Runtime / Integrated Server arc in [`README.md`](README.md), starting with [`ClientRuntime0-integrated-server-client-world-boundary.md`](ClientRuntime0-integrated-server-client-world-boundary.md).

## Constraints To Carry Forward

Future movement work should still preserve:

- sequenced command records, not latest-input authority
- host acks of the last processed command sequence
- fixed command quanta for player movement prediction
- snap-and-replay simulation truth with presentation-only smoothing
- no render-thread ownership of replay buffers, collision chunks, or host state
- NPC AI and player body stepping as related but separately schedulable systems

## Resume Criteria

Redraft movement only after the client runtime arc defines:

- `IntegratedServer` / dedicated host naming and construction
- `ClientRuntime` and `ClientWorld` ownership
- singleplayer and multiplayer client-world hydration
- render/presentation thread boundaries
- a prediction-service API over bounded client-world collision/entity views

The next movement tactical should be a new document, not an edit of this paused slice.
