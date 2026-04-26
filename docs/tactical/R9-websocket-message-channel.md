# R9: WebSocket Message Channel

Standing after `R0` through `R8` and the `ClientRuntime0` through `ClientRuntime6` arc. The current remote path has the right host/client ownership shape, but its carrier is still HTTP polling. Before grounding player physics and higher-frequency entity updates on the remote path, move dedicated remote play to a persistent WebSocket-backed message channel.

## Goal

Replace the remote HTTP polling adapter with a WebSocket transport that carries the same logical host/client records:

- open/create world, join/resume session, and named player-slot identity
- chunk interest commands and authoritative chunk/entity/player/session updates
- sequenced player input commands and authoritative ack snapshots
- reconnect/resume behavior without treating a random session id as the player slot

This is a transport migration, not a gameplay protocol rewrite.

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | remote wire codec | split reusable message serialization from HTTP-specific request/response envelopes |
| 2 | WebSocket server | dedicated Node host upgrades clients to one persistent ordered channel per session |
| 3 | WebSocket client transport | browser `WorldTransport` implementation sends command envelopes and queues server-pushed host messages |
| 4 | request/reply envelope | commands that need an acknowledgement carry request ids; pushed updates do not require polling |
| 5 | update pump | server drains authoritative host/session pending messages and pushes them to visible/interested clients |
| 6 | compatibility removal path | keep HTTP only until WebSocket browser/runtime tests replace equivalent coverage, then remove `world-http-protocol.ts` and `RemoteWorldTransport` |

Do not add:

- WebRTC/WebTransport/datagram lanes
- lossy or unordered movement traffic
- a new gameplay authority model
- grounded movement physics
- persisted player inventory/state beyond the join/player-slot identity needed by the transport

## Protocol Shape

Use one WebSocket connection per client session. Initial connect may include a resume session id; otherwise the first command opens/joins a world and allocates a session.

Suggested JSON control frames:

```ts
type ClientFrame =
  | { kind: "request"; requestId: number; message: WorldClientMessage }
  | { kind: "ack"; receivedSequence: number };

type ServerFrame =
  | { kind: "response"; requestId: number; messages: readonly WorldHostMessage[] }
  | { kind: "push"; sequence: number; messages: readonly WorldHostMessage[] }
  | { kind: "error"; requestId?: number; code: string; message: string };
```

The first slice can keep packed chunk/light payloads in the existing JSON-safe representation. A follow-up can move bulk payloads to binary frames once the channel semantics are stable.

## Migration Notes

- `ClientRuntime.drainTransportUpdates()` can remain as the presentation-loop drain point, but WebSocket transports should drain an in-memory pushed-message queue rather than making a network `poll_world_updates` call.
- `GeneratedWorldRemoteService` is reusable as the shared authority/session owner, but it needs a push pump so player/session/chunk updates are delivered when ticks or chunk jobs enqueue messages.
- `world-http-protocol.ts` should not survive as a generic dependency. Keep reusable `serializeWorldClientMessage` / `serializeWorldHostMessages`-style functions in a transport-neutral remote wire module, and keep HTTP-specific envelope types only while the HTTP adapter still exists.
- The WebSocket migration should land before movement physics relies on remote prediction parity, so player snapshots and command acks are no longer paced by HTTP polling.

## Validation

Minimum:

- `pnpm test -- test/runtime/remote-world-transport.test.ts` or its WebSocket replacement
- `pnpm test:browser`
- `pnpm test:browser:integration` for debug camera/player state and two-client remote coverage
- `git diff --check`

## Done When

- Browser remote clients use WebSocket by default.
- Server-originated chunk, entity, session, and player updates arrive without `poll_world_updates` network requests.
- Two browser clients can join one shared dedicated world through the WebSocket path.
- Resume/reconnect tests cover the same cases as the old HTTP transport.
- HTTP polling code is either removed or explicitly isolated as a temporary fallback with no runtime/browser default path.
