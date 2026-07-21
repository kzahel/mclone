import { chooseDefaultClip } from "./catalog-model";
import { parseFigureAssetJson } from "./figure-json";
import { FigureViewportController } from "./viewport";

declare global {
  interface Window {
    assetLabRenderThumbnail?: (
      figureJson: string,
      sourceLabel: string,
      defaultClip?: string,
    ) => Promise<void>;
    assetLabThumbnailReady?: boolean;
  }
}

const host = document.querySelector<HTMLElement>("#thumbnail");
if (!host) {
  throw new Error("Missing #thumbnail");
}
const viewport = new FigureViewportController(host, {
  background: "#edf1f4",
  pixelRatioCap: 1,
  showFloor: false,
});
viewport.setPlaying(false);

window.assetLabRenderThumbnail = async (figureJson, sourceLabel, requestedClip) => {
  const asset = parseFigureAssetJson(figureJson, sourceLabel);
  const clipName = requestedClip ?? chooseDefaultClip(Object.keys(asset.clips));
  viewport.setAsset(asset, clipName);
  viewport.setPlaying(false);
  const duration = viewport.getDuration();
  viewport.setTime(duration * 0.18);
  viewport.setCameraPreset("three-quarter");
  await nextPaint();
  await nextPaint();
};
window.assetLabThumbnailReady = true;

function nextPaint(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}
