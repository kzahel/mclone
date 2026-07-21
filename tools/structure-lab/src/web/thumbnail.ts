import type { StructureCatalogEntry } from "../catalog-model";
import { StructureViewportController } from "../viewport";

declare global {
  interface Window {
    structureLabRenderThumbnail?: (entry: StructureCatalogEntry) => Promise<void>;
    structureLabThumbnailReady?: boolean;
  }
}

const host = document.querySelector<HTMLElement>("#thumbnail");
if (!host) throw new Error("Missing Structure Lab thumbnail host");

const viewport = new StructureViewportController(host, { background: "light" });
window.structureLabRenderThumbnail = async (entry) => {
  await viewport.load(entry);
  viewport.setBoundsVisible(false);
  viewport.setMarkersVisible(false);
  viewport.setCameraPreset("three-quarter");
  viewport.renderNow();
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
};
window.structureLabThumbnailReady = true;
