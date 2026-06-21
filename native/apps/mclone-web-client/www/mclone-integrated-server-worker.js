let wasmModulePromise = null;
let server = null;
let tickTimer = 0;

self.onmessage = async (event) => {
  const message = event.data ?? {};
  try {
    switch (message.kind) {
      case "start":
        await startServer(message);
        break;
      case "command":
        await handleCommand(message);
        break;
      case "shutdown":
        shutdown(message);
        break;
      default:
        postFailure(
          message.requestId,
          `unexpected integrated server worker message kind ${String(message.kind)}`,
        );
        break;
    }
  } catch (error) {
    postFailure(message.requestId, stringifyError(error));
  }
};

async function startServer(message) {
  if (server) {
    postFailure(message.requestId, "integrated server worker was already started");
    return;
  }

  const module = await loadWasmModule(message.bindgenJsUrl, message.bindgenWasmUrl);
  server = message.jobWorkerUrl && typeof module.McloneWebIntegratedServerWorker.withJobWorkers === "function"
    ? module.McloneWebIntegratedServerWorker.withJobWorkers(
      toBigIntSeed(message.seed),
      String(message.jobWorkerUrl),
      String(message.bindgenJsUrl),
      String(message.bindgenWasmUrl),
    )
    : new module.McloneWebIntegratedServerWorker(toBigIntSeed(message.seed));
  const intervalMs = Math.max(1, Number(message.tickIntervalMs) || 50);
  tickTimer = setInterval(tickServer, intervalMs);
  const diagnostics = server.diagnostics();
  self.postMessage({
    ok: true,
    kind: "ready",
    requestId: Number(message.requestId) || 0,
    updates: [],
    diagnostics,
  });
}

async function handleCommand(message) {
  if (!server) {
    postFailure(message.requestId, "integrated server worker is not started");
    return;
  }
  const frame = message.frame instanceof Uint8Array ? message.frame : new Uint8Array();
  const result = server.handleCommandFrame(frame);
  const updates = Array.isArray(result.updates) ? [...result.updates] : [];
  let diagnostics = result.diagnostics;
  for (let attempt = 0; hasPendingServerJobs(diagnostics) && attempt < 60000; attempt += 1) {
    await waitForJobTurn();
    const poll = server.poll();
    if (Array.isArray(poll.updates)) {
      updates.push(...poll.updates);
    }
    diagnostics = poll.diagnostics;
  }
  if (hasPendingServerJobs(diagnostics)) {
    postFailure(message.requestId, "timed out waiting for web integrated server jobs");
    return;
  }
  postUpdates({
    ok: true,
    kind: "command-result",
    requestId: Number(message.requestId) || 0,
    updates,
    diagnostics,
  });
}

function tickServer() {
  if (!server) return;
  try {
    const result = server.tick();
    if (Number(result.updateCount) > 0) {
      postUpdates({
        ok: true,
        kind: "updates",
        requestId: 0,
        updates: result.updates,
        diagnostics: result.diagnostics,
      });
    }
  } catch (error) {
    postFailure(0, stringifyError(error));
  }
}

function shutdown(message) {
  if (tickTimer) {
    clearInterval(tickTimer);
    tickTimer = 0;
  }
  const result = server?.shutdown?.() ?? { updates: [], diagnostics: null };
  server = null;
  self.postMessage({
    ok: true,
    kind: "shutdown-complete",
    requestId: Number(message.requestId) || 0,
    updates: [],
    diagnostics: result.diagnostics,
  });
}

function postUpdates(message) {
  const updates = Array.isArray(message.updates) ? message.updates : [];
  const transfers = [];
  for (const update of updates) {
    if (update instanceof Uint8Array) {
      transfers.push(update.buffer);
    }
  }
  self.postMessage(
    {
      ...message,
      updates,
      updateCount: updates.length,
    },
    transfers,
  );
}

function postFailure(requestId, reason) {
  self.postMessage({
    ok: false,
    kind: "error",
    requestId: Number(requestId) || 0,
    reason,
  });
}

function loadWasmModule(bindgenJsUrl, bindgenWasmUrl) {
  wasmModulePromise ??= import(bindgenJsUrl).then(async (module) => {
    await module.default(bindgenWasmUrl);
    return module;
  });
  return wasmModulePromise;
}

function hasPendingServerJobs(diagnostics) {
  if (!diagnostics) return false;
  return (
    Number(diagnostics.pendingJobs) > 0
    || Number(diagnostics.pendingPublications) > 0
    || Number(diagnostics.worldgenMailboxPendingJobs) > 0
    || Number(diagnostics.lightStatusMailboxPendingStatuses) > 0
  );
}

function waitForJobTurn() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function toBigIntSeed(seed) {
  const number = Number(seed);
  return BigInt(Number.isFinite(number) ? Math.trunc(number) : 0);
}

function stringifyError(error) {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}
