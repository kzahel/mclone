// Explicit test-client exposure for browser smoke probes. The ordinary page
// imports this module only when `smokeObserver=1` is present in the query.
// Explorer vocabulary is permitted here because this is not the production
// platform adapter.

export function installWorldExplorerSmokeObserver(session, failWorker) {
  let presentedFrames = 0;
  const observer = Object.freeze({
    frame: () => presentedFrames,
    snapshot: () => (
      presentedFrames === 0 ? null : JSON.parse(session.diagnosticSnapshot())
    ),
  });
  const commands = Object.freeze({
    recenter: (worldX, worldZ) => session.recenterForSmoke(worldX, worldZ),
    failWorker,
    shutdown: () => session.shutdown(),
    shutdownComplete: () => session.shutdownComplete(),
  });
  globalThis.__MCLONE_WORLD_EXPLORER_SMOKE__ = Object.freeze({
    observer,
    commands,
  });
  return Object.freeze({
    observeFrame() {
      presentedFrames += 1;
    },
  });
}
