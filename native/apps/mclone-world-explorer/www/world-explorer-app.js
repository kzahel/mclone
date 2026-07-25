const shell = document.getElementById("world-explorer-shell");
const canvas = document.getElementById("world-explorer-canvas");
const status = document.getElementById("world-explorer-status");

if (!(shell instanceof HTMLElement)
    || !(canvas instanceof HTMLCanvasElement)
    || !(status instanceof HTMLOutputElement)) {
  throw new Error("World Explorer document is missing its required elements");
}

const runtime = {
  frame: 0,
  report: null,
  session: null,
};
globalThis.__MCLONE_WORLD_EXPLORER__ = runtime;

void boot().catch((error) => {
  shell.dataset.failed = "true";
  shell.setAttribute("aria-busy", "false");
  status.value = error instanceof Error ? error.stack ?? error.message : String(error);
  console.error(error);
});

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
  runtime.session = await module.mclone_world_explorer_create(
    canvas,
    authored,
    provisional,
    new Uint8Array(),
    globalThis.location.search,
  );
  bindRawObservations();
  shell.setAttribute("aria-busy", "false");
  canvas.focus({ preventScroll: true });
  requestAnimationFrame(renderFrame);
}

function renderFrame() {
  if (!runtime.session || document.visibilityState === "hidden") {
    requestAnimationFrame(renderFrame);
    return;
  }
  syncCanvasSize();
  const report = JSON.parse(runtime.session.renderFrame());
  runtime.frame += 1;
  runtime.report = report;
  shell.dataset.ready = String(Boolean(report.targetReady));
  shell.dataset.revision = String(report.revision);
  shell.dataset.allocationSlots = String(report.allocationSlots);
  shell.dataset.readySlots = String(report.readySlots);
  shell.dataset.pendingRefills = String(report.pendingRefills);
  shell.dataset.totalRefills = String(report.totalRefills);
  shell.dataset.totalRebases = String(report.totalRebases);
  shell.dataset.centerX = String(report.centerX);
  shell.dataset.centerZ = String(report.centerZ);
  status.value = [
    `seed ${report.seed} · ${report.view} · ${report.blocksAcross} blocks`,
    `center ${report.centerX}, ${report.centerZ}`,
    `${report.readySlots}/${report.allocationSlots} fixed slots · ${report.pendingRefills} terrain pending`,
    `${report.drawnLevels} levels · ${report.drawnTiles} draws · ${report.treeInstanceCount} trees`,
    `${(report.fixedResidentBytes / 1048576).toFixed(1)} MiB fixed terrain · ${report.pendingVegetationTiles} vegetation pending`,
  ].join("\n");
  requestAnimationFrame(renderFrame);
}

function bindRawObservations() {
  canvas.addEventListener("pointerdown", (event) => {
    const point = localPoint(event);
    canvas.focus({ preventScroll: true });
    canvas.setPointerCapture(event.pointerId);
    runtime.session.pointerDown(
      event.pointerId,
      event.button,
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
