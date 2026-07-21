import { loadBrowserFigure } from "./browser-load";
import { FigureViewportController } from "./viewport";

declare global {
  interface Window {
    assetLabReady?: boolean;
  }
}

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) {
  throw new Error("Missing #app");
}

try {
  const params = new URLSearchParams(window.location.search);
  const figurePath = params.get("figure") ?? "/examples/chicken/figure.ts";
  const clipName = params.get("clip") ?? "walk";
  const asset = await loadBrowserFigure(figurePath);
  const activeClip = asset.clips[clipName] ? clipName : undefined;
  const viewport = new FigureViewportController(app);
  viewport.setAsset(asset, activeClip);
  window.assetLabReady = true;
} catch (error) {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  app.innerHTML = `<pre class="error"></pre>`;
  const pre = app.querySelector("pre");
  if (pre) {
    pre.textContent = message;
  }
  window.assetLabReady = true;
}
