import { connectRenderWorldWorkerSession, type RenderWorldWorkerHostEndpoint } from "./render-world-worker-client";
import { createRenderWorldWorkerHandler } from "./render-world-worker-handler";

export * from "./render-world-worker-handler";

const maybeWorkerGlobal = globalThis as Partial<RenderWorldWorkerHostEndpoint>;
if (typeof maybeWorkerGlobal.postMessage === "function" && typeof maybeWorkerGlobal.addEventListener === "function") {
  connectRenderWorldWorkerSession(
    maybeWorkerGlobal as RenderWorldWorkerHostEndpoint,
    createRenderWorldWorkerHandler(),
  );
}
