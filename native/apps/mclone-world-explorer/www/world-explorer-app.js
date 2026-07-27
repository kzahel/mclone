import { PolledWorkerTransport } from "./mclone-worker-transport.js";

const shell = document.getElementById("world-explorer-shell");
const canvas = document.getElementById("world-explorer-canvas");
const status = document.getElementById("world-explorer-status");

if (!(shell instanceof HTMLElement)
    || !(canvas instanceof HTMLCanvasElement)
    || !(status instanceof HTMLOutputElement)) {
  throw new Error("World Explorer document is missing its required elements");
}

const runtime = {
  session: null,
  horizonTransport: null,
  exactTransport: null,
};
let smokeObserver = null;

void boot().catch(showFatalError);

async function boot() {
  const module = await import("./pkg/mclone_world_explorer.js");
  await module.default(new URL(
    "./pkg/mclone_world_explorer_bg.wasm",
    globalThis.location.href,
  ));
  const [authored, provisional] = await Promise.all([
    fetchBytes("./first-party-packs/mclone-authored.pbp"),
    fetchBytes("./first-party-packs/mclone-generated-fallback.pbp"),
  ]);
  syncCanvasSize();
  const horizonTransportFactory = () => {
    runtime.horizonTransport = new PolledWorkerTransport(
      new URL("./world-explorer-worker.js", import.meta.url),
      "mclone-world-explorer-worker",
    );
    return runtime.horizonTransport;
  };
  const exactTransportFactory = () => {
    runtime.exactTransport = new PolledWorkerTransport(
      new URL("./world-explorer-exact-worker.js", import.meta.url),
      "mclone-world-explorer-exact-worker",
    );
    return runtime.exactTransport;
  };
  runtime.session = await module.mclone_world_explorer_create(
    canvas,
    authored,
    provisional,
    new Uint8Array(),
    globalThis.location.search,
    horizonTransportFactory,
    exactTransportFactory,
  );
  await installSmokeObserverIfRequested();
  bindRawObservations();
  shell.setAttribute("aria-busy", "false");
  status.hidden = true;
  canvas.focus({ preventScroll: true });
  requestAnimationFrame(renderFrame);
}

function renderFrame(frameMillis) {
  if (!runtime.session || document.visibilityState === "hidden") {
    requestAnimationFrame(renderFrame);
    return;
  }
  syncCanvasSize();
  try {
    runtime.session.renderFrame(frameMillis);
  } catch (error) {
    showFatalError(error);
    return;
  }
  smokeObserver?.observeFrame();
  requestAnimationFrame(renderFrame);
}

async function installSmokeObserverIfRequested() {
  const parameters = new URLSearchParams(globalThis.location.search);
  if (parameters.get("smokeObserver") !== "1") {
    return;
  }
  const observer = await import("./world-explorer-smoke-observer.js");
  smokeObserver = observer.installWorldExplorerSmokeObserver(
    runtime.session,
    () => runtime.horizonTransport?.terminate(),
  );
}

function showFatalError(error) {
  shell.dataset.failed = "true";
  shell.setAttribute("aria-busy", "false");
  status.hidden = false;
  status.value = error instanceof Error ? error.stack ?? error.message : String(error);
  console.error(error);
}

function bindRawObservations() {
  canvas.addEventListener("pointerdown", (event) => {
    const point = localPoint(event);
    canvas.focus({ preventScroll: true });
    canvas.setPointerCapture(event.pointerId);
    runtime.session.pointerDown(
      event.pointerId,
      event.button,
      event.shiftKey,
      point.x,
      point.y,
      event.timeStamp / 1000,
      point.width,
      point.height,
    );
    event.preventDefault();
  });
  canvas.addEventListener("pointermove", (event) => {
    if (!canvas.hasPointerCapture(event.pointerId)) {
      return;
    }
    const point = localPoint(event);
    runtime.session.pointerMove(
      event.pointerId,
      point.x,
      point.y,
      event.timeStamp / 1000,
      point.width,
      point.height,
    );
    event.preventDefault();
  });
  const endPointer = (event) => {
    const point = localPoint(event);
    runtime.session.pointerUp(
      event.pointerId,
      point.x,
      point.y,
      event.timeStamp / 1000,
      point.width,
      point.height,
    );
    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
    event.preventDefault();
  };
  canvas.addEventListener("pointerup", endPointer);
  canvas.addEventListener("pointercancel", () => runtime.session.cancelInput());
  canvas.addEventListener("contextmenu", (event) => event.preventDefault());
  canvas.addEventListener("wheel", (event) => {
    const point = localPoint(event);
    runtime.session.wheel(
      event.deltaY,
      event.deltaMode,
      point.x,
      point.y,
      point.width,
      point.height,
    );
    event.preventDefault();
  }, { passive: false });
  globalThis.addEventListener("keydown", (event) => {
    if (runtime.session.rawKey(event.code, true, event.repeat)) {
      event.preventDefault();
    }
  });
  globalThis.addEventListener("keyup", (event) => {
    if (runtime.session.rawKey(event.code, false, false)) {
      event.preventDefault();
    }
  });
  globalThis.addEventListener("blur", () => runtime.session.cancelInput());
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") {
      runtime.session.cancelInput();
    }
  });
  globalThis.addEventListener("pagehide", () => runtime.session?.shutdown());
  globalThis.addEventListener("resize", syncCanvasSize);
}

function syncCanvasSize() {
  const bounds = canvas.getBoundingClientRect();
  const scale = Math.max(1, globalThis.devicePixelRatio || 1);
  const width = Math.max(1, Math.round(bounds.width * scale));
  const height = Math.max(1, Math.round(bounds.height * scale));
  if (runtime.session && (canvas.width !== width || canvas.height !== height)) {
    runtime.session.resize(width, height);
  } else if (!runtime.session) {
    canvas.width = width;
    canvas.height = height;
  }
}

function localPoint(event) {
  const bounds = canvas.getBoundingClientRect();
  return {
    x: event.clientX - bounds.left,
    y: event.clientY - bounds.top,
    width: Math.max(1, bounds.width),
    height: Math.max(1, bounds.height),
  };
}

async function fetchBytes(url, optional = false) {
  const response = await fetch(url);
  if (!response.ok) {
    if (optional && response.status === 404) {
      return new Uint8Array();
    }
    throw new Error(`Failed to fetch ${url}: ${response.status} ${response.statusText}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}
