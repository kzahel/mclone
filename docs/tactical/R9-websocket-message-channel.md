# R9: WebSocket Message Channel

Status: done.

Standing after `R0` through `R8` and the `ClientRuntime0` through `ClientRuntime6` arc. The current remote path has the right host/client ownership shape, but its carrier is still HTTP polling. Before grounding player physics and higher-frequency entity updates on the remote path, move dedicated remote play to a persistent WebSocket-backed message channel.

Implementation landed:

- remote browser clients use `RemoteWorldWebSocketTransport` by default
- `src/runtime/protocol/world-wire-protocol.ts` owns reusable remote message serialization
- `world-http-protocol.ts` is now an HTTP envelope wrapper over the shared wire codec
- `GeneratedWorldHttpServer` accepts `/api/world/socket` upgrades and pushes queued service messages through one ordered JSON channel per connection
- `ClientRuntime.drainTransportUpdates()` remains the hydrate/drain point; WebSocket clients drain an in-memory pushed-message queue instead of issuing network `poll_world_updates`
- HTTP polling remains only as non-default compatibility coverage

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
| 1 | remote wire codec | done - reusable message serialization is split from HTTP envelopes |
| 2 | WebSocket server | done - dedicated Node host upgrades clients to one persistent ordered channel per session |
| 3 | WebSocket client transport | done - browser `WorldTransport` implementation sends command envelopes and queues server-pushed host messages |
| 4 | request/reply envelope | done - commands carry request ids; pushed updates do not require polling |
| 5 | update pump | done - server drains authoritative host/session pending messages and pushes them to visible/interested clients |
| 6 | compatibility removal path | partially done - HTTP is non-default compatibility coverage; deletion can happen after follow-up coverage no longer needs the adapter |

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

Completed:

- `pnpm test -- test/runtime/remote-world-transport.test.ts`
- `pnpm typecheck`
- `pnpm test:browser`
- `git diff --check`

## Done When

- Done: browser remote clients use WebSocket by default.
- Done: server-originated chunk, entity, session, and player updates arrive without `poll_world_updates` network requests.
- Done: two browser clients can join one shared dedicated world through the WebSocket path.
- Done: resume/reconnect tests cover the same cases as the old HTTP transport.
- Done: HTTP polling code is isolated as a temporary fallback with no runtime/browser default path.

## Follow-Up

The next Tactical 53 dependency is join/player-slot semantics: keep `sessionId` as a connection/resume handle, introduce explicit join/resume identity facts, and stop depending on `playerId === sessionId` before grounded movement uses player identity.
