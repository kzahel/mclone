# Browser-Hosted Peer Sessions

Topic: `browser-hosted-peer-sessions`

Status: **accepted direction; not implemented** as of 2026-07-23.

## Purpose

Define how a browser can host an authoritative Mclone room without accepting
an inbound WebTransport connection. This topic owns browser peer-session
topology, signaling, WebRTC carrier shape, worker ownership, and host lifecycle
limits.

It complements:

- [`multiplayer-networking.md`](multiplayer-networking.md), which owns shared
  authority, connection, and transport boundaries;
- [`remote-player-presentation.md`](remote-player-presentation.md), which owns
  pose cadence, interpolation, and reliable/ephemeral message semantics;
- [`../multiplayer-hosting.md`](../multiplayer-hosting.md), which describes the
  player-facing hosting modes.

## Corrected Premise

A browser cannot listen for inbound WebTransport connections. It can still
host multiplayer:

1. The host tab runs the existing Rust `RealmServer` in its worker, just as
   browser singleplayer does.
2. A lightweight signaling service maps a short room key to the participants
   and forwards offer, answer, ICE-candidate, and room metadata.
3. The browsers establish one WebRTC peer connection per guest.
4. Gameplay crosses WebRTC data channels and attaches to the ordinary
   `RealmServer` connection registry.

The signaling service is not authoritative and does not receive world state.
After a direct path succeeds it is no longer in the gameplay data path.

This is an asymmetric peer topology: the browser host owns authority and every
guest is a client. It is not a distributed or lockstep simulation.

## Carrier Contract

Each guest connection should expose the same two logical lanes used by the
mixed-reliability WebTransport design:

```js
const reliable = peer.createDataChannel("mclone-reliable", {
  ordered: true,
});
const ephemeral = peer.createDataChannel("mclone-ephemeral", {
  ordered: false,
  maxRetransmits: 0,
});
```

The reliable lane carries login/session state, chunks, inventory, interactions,
teleports, respawn, persistence barriers, and any pose barrier required to
interpret a gameplay command. The ephemeral lane carries bounded,
self-recovering, sequenced pose samples for which newer data supersedes older
data.

The decoded logical protocol remains carrier-neutral. WebRTC must not create a
second set of gameplay messages, authority rules, or presentation semantics.
Cross-lane ordering uses the same explicit epochs/sequences/barriers as
WebTransport; arrival time on two independent channels is not an ordering
guarantee.

## Browser Ownership

`RTCPeerConnection` is a Window API. The page-side broker therefore owns:

- room creation/join and signaling requests;
- peer-connection creation and ICE configuration;
- offer/answer/candidate exchange;
- channel-open, connection-state, and selected-candidate diagnostics.

Current WebRTC specifies `RTCDataChannel` as transferable to a dedicated
worker. The preferred path transfers opened channels to the existing
Rust/WASM server or remote-client worker. Implementations must feature-detect
that capability. The compatibility path keeps the channel in a small
page-side broker and forwards binary `ArrayBuffer` payloads across the worker
boundary.

That compatibility broker stays domain-blind: it moves bounded bytes and
pressure/disconnect signals. Protocol decode, world authority, input, and
simulation do not move onto the page or render thread.

## Signaling, STUN, and TURN

A room key is a capability to enter signaling, not proof that all gameplay
will be peer-to-peer:

- Signaling exchanges session descriptions and incremental ICE candidates.
- STUN helps peers discover usable public-facing candidates.
- ICE selects a working direct path when one exists.
- TURN is required for network combinations that cannot form a direct path.

When TURN is selected, the TURN server relays gameplay traffic. It therefore
has bandwidth, regional latency, abuse-control, credential, and operating-cost
requirements beyond a lightweight room-key signaling service.

The UI and diagnostics should report at least `direct`, `relayed`, and
`unknown` connection modes. Product language must not promise serverless or
relay-free internet play. Development may start with signaling plus public
STUN, but release-quality joins need a deliberate TURN availability policy.

## Host Lifecycle and Scale

Browser authority has stricter lifecycle limits than a dedicated or native
integrated host:

- closing, navigating, crashing, or refreshing the host tab ends the session;
- mobile backgrounding and browser throttling may stall the authority;
- the host's IndexedDB owns the world unless it is explicitly exported;
- initial implementation has no host migration;
- each guest adds another peer connection and another copy of outbound state.

The initial product should target small friend rooms in a host-centered star,
declare the host visibly, warn before destructive navigation where possible,
and produce an explicit host-lost disconnect reason. It must not silently
pretend that another guest can continue the world.

Autosave and lifecycle flush remain the ordinary browser integrated-server
responsibility. Networking must not add a second persistence path.

## Platform Boundary

Browser-to-browser rooms use the browser's built-in WebRTC implementation and
do not add a compiled WebRTC dependency to Mclone's WASM or native binaries.

Native clients joining a browser-hosted room are a separate milestone. They
would require either:

- a native WebRTC data-channel adapter with ICE/DTLS/SCTP and TURN support; or
- a gateway that terminates WebRTC and forwards the neutral Mclone protocol.

That dependency and deployment choice must be measured independently.
Dedicated and native-integrated hosts expose the same carrier-neutral logical
lanes through native TCP-plus-UDP, compatibility transports, or a future
WebTransport adapter. Browser-hosted WebRTC is an additional topology, not a
replacement for those client/server profiles.

## Implementation Sequence

1. Finish the shared reliable/ephemeral lane, pose sequencing, and cross-lane
   barrier contracts independently of any carrier.
2. Add a minimal authenticated room-key signaling protocol and expiring room
   state.
3. Add a page-side peer broker with explicit connection diagnostics and
   transferable-data-channel feature detection.
4. Attach guest channels to the worker-owned `RealmServer`, using a bounded
   byte-forwarding fallback when channel transfer is unavailable.
5. Add ICE-server configuration, short-lived TURN credentials, and direct
   versus relayed reporting.
6. Validate Chrome, Firefox, Safari, Android browser/WebView, and Quest Browser
   across direct LAN, ordinary NAT, forced TURN, backgrounding, host exit,
   packet loss, backpressure, and multiple guests.
7. Consider native WebRTC participation and host migration only as separate
   measured work streams.

## Accepted Decisions

- Browser-hosted authoritative rooms are a supported target.
- The existing worker-owned Rust `RealmServer` remains authority.
- Room-key signaling carries negotiation metadata, not gameplay or world
  authority.
- A reliable ordered and an unordered zero-retransmit data channel implement
  the shared logical lane contract.
- TURN is a conditional gameplay relay, not merely signaling.
- The page-side peer broker remains a transport adapter; simulation and
  protocol policy stay in shared Rust owners.
- Initial rooms are small host-centered stars without host migration.
- Browser WebRTC and client/server carriers such as native TCP-plus-UDP and
  WebTransport solve different connection topologies and should coexist
  behind the same logical lanes.

## Open Decisions

- Signaling service deployment, authentication, room expiry, and abuse limits.
- TURN provider/deployment, credential issuance, and relay availability
  promise.
- Maximum supported guest count and per-peer send budgets.
- Whether transferable data channels are a required baseline or only an
  optimization for each browser product.
- Host-background policy: pause with a visible warning, tolerate a grace
  interval, or disconnect immediately.
- World export/import and later host migration UX.
- Whether native participation uses an embedded WebRTC stack or a gateway.

## External Receipts

- Chrome's
  [Chrome 97 announcement](https://developer.chrome.com/blog/new-in-chrome-97/)
  states that WebTransport supports reliable streams and unreliable datagrams,
  works in workers, and is a client/server API.
- Chrome's
  [WebTransport guide](https://developer.chrome.com/docs/capabilities/web-apis/webtransport)
  distinguishes WebTransport client/server use from WebRTC peer-to-peer use
  and documents Chromium datagrams.
- The
  [WebRTC specification](https://www.w3.org/TR/webrtc/)
  defines `RTCPeerConnection`, ordered/unordered data channels,
  `maxRetransmits`, and transferable worker-visible `RTCDataChannel`.
- The
  [WebTransport specification](https://www.w3.org/TR/webtransport/)
  distinguishes HTTP/3's unreliable support from reliable-only HTTP/2
  fallback through the newer `reliability` and `requireUnreliable` API
  members.

## Code Map

- `native/apps/mclone-web-client/src/web_server_worker.rs`: existing
  worker-owned browser `RealmServer`.
- `native/apps/mclone-web-client/src/web_remote_worker_actor.rs`: existing
  remote-client worker transport ownership pattern.
- `native/apps/mclone-web-client/www/`: page-side browser glue and future peer
  broker/signaling owner.
- `native/crates/mclone-server`: authoritative realm and connection registry.
- `native/crates/mclone-app-runtime`: transport-neutral connection boundary.
- `native/crates/mclone-protocol`: logical reliable and ephemeral messages.
