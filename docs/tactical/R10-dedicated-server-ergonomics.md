# R10: Dedicated Server Ergonomics

Status: active - script alias landed.

`R9` landed the persistent WebSocket remote channel. The next small multiplayer-facing cleanup is not another transport experiment. It is making the dedicated server shape clear and testable for humans:

- call the server process `host:dedicated`, not only `host:remote`
- keep `host:remote` as a compatibility alias for existing docs/tests
- make config-file startup a first-class tested path
- give the browser UI/query string an explicit way to choose local singleplayer vs dedicated WebSocket
- record reload/static-hosting/P2P expectations without implementing them in this slice

Durable architecture: [`../multiplayer-hosting.md`](../multiplayer-hosting.md).

## Goal

Make the current WebSocket dedicated host feel like a named server mode instead of a transport implementation detail.

## Scope

Add or update:

| # | Area | Expected result |
|---|---|---|
| 1 | npm scripts | done - `host:dedicated` starts `src/runtime/node/generated-world-http-server.ts`; `host:remote` remains as a compatibility alias |
| 2 | docs/help output | docs and server startup JSON use "dedicated" wording where user-facing |
| 3 | config-file test | spawned server can start from `--config <path>` with host/port/saveRoot and serve `/healthz` |
| 4 | WebSocket config integration test | `RemoteWorldWebSocketTransport` can open a world against a server started from config |
| 5 | browser query aliases | parse `worldAuthority=local|dedicated`, `dedicatedHostUrl`, and `netTransport=websocket` as aliases for the current remote path |
| 6 | GUI settings | add a title/debug settings control for Local Singleplayer vs Dedicated Server and a dedicated-server URL input |
| 7 | browser smoke | focused browser smoke proves a query/UI-selected dedicated WebSocket session reaches `worldTransport=remote` and loads the expected chunk ring |

Preserve:

- `worldTransport=worker|remote`
- `worldHostUrl`
- existing Playwright remote fixtures
- current WebSocket default for remote clients

## Out Of Scope

- WebRTC implementation
- P2P browser-host mode
- multi-world protocol changes
- static `dist/` hosting from the dedicated process
- authenticated admin/RCON commands
- watch-mode or graceful restart implementation
- renaming internal `GeneratedWorldHttpServer` classes/files unless the alias work is already complete and the rename is very low-risk

## Query/UI Shape

Current supported shape:

```text
/?mode=debug&worldTransport=remote&worldHostUrl=http://127.0.0.1:4173
```

Add aliases:

```text
/?mode=debug&worldAuthority=dedicated&dedicatedHostUrl=http://127.0.0.1:4173&netTransport=websocket
/?mode=debug&worldAuthority=local
```

Rules:

- `worldTransport` remains authoritative if both old and new params are present in a conflicting way, until migration is complete.
- `netTransport=websocket` is the only accepted dedicated transport for this slice.
- Unknown `netTransport` values should fall back to WebSocket or report a clear UI error; do not silently imply WebRTC support.
- UI should persist the selected authority and URL in localStorage, but direct query params should override stored state.
- The title/debug settings screen should make "Dedicated Server" visibly distinct from local singleplayer before starting the world.

## Config Test Shape

Use a temp config file like:

```json
{
  "host": "127.0.0.1",
  "port": 0,
  "saveRoot": "/tmp/mclone-dedicated-test"
}
```

Preferred tests:

- unit-level parser test if config loading is exported cleanly
- spawned-process integration test that reads the server startup JSON and checks:
  - `type === "listening"`
  - URL is reachable
  - `/healthz` returns `ok: true`
  - `saveRoot` resolves to the configured temp directory
- runtime transport test that opens a WebSocket session against that URL and receives `world_opened` plus `session_state`

If `port: 0` is not supported by the current CLI/config parser, add support as part of this slice so tests do not race fixed ports.

## Dedicated Server Config Direction

This slice may keep the existing host/port/saveRoot config shape. Follow-up work should extend the config toward:

- `schemaVersion`
- `staticRoot`
- `worlds[]`
- `defaultWorld`
- per-world seed/preset/engine config
- transport feature flags
- reload/watch policy

Do not let browser query params become the long-term dedicated-server world-definition mechanism.

## Deferred Reload/Restart Notes

Future reload work should be admin-only:

- `--watch-config` marks config dirty
- health/admin state reports reload pending
- authenticated admin command requests graceful reload or process restart
- graceful reload flushes storage and tells clients to reconnect/resume

Do not add an unauthenticated browser command that can restart a dedicated server.

## Validation

Minimum:

```bash
pnpm test -- test/runtime/remote-world-transport.test.ts
pnpm typecheck
```

When GUI/query work is included and a browser/WebGPU host is available:

```bash
pnpm test:browser -- test/browser/smoke.test.ts
```

If the host has no browser/WebGPU lane, document that and run the config/runtime transport tests plus `pnpm typecheck`.

## Done When

- `pnpm host:dedicated` starts the current dedicated WebSocket-capable Node server.
- `pnpm host:remote` still works.
- A config-file spawned dedicated server is covered by tests.
- Browser query aliases can select local vs dedicated authority.
- The GPU title/debug settings UI can start a dedicated WebSocket session without editing the URL by hand.
- Existing remote browser tests keep passing through the old query params during the migration.
