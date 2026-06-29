import * as THREE from "three";
import { assertValidFigure, type FigureAsset } from "./dsl";
import { createReviewFloor, locomotionSummary, type ReviewFloor } from "./review-floor";
import { clipDuration, createFigureScene, type FigureScene } from "./scene";

declare global {
  interface Window {
    assetLabSetVideoTime?: (timeSeconds: number) => void;
    assetLabVideoDuration?: number;
    assetLabVideoReady?: boolean;
  }
}

type ViewName = "right" | "three-quarter";

interface RenderPanel {
  camera: THREE.PerspectiveCamera;
  figureScene: FigureScene;
  floor: ReviewFloor;
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
}

const video = document.querySelector<HTMLElement>("#video");
if (!video) {
  throw new Error("Missing #video");
}

try {
  const params = new URLSearchParams(window.location.search);
  const figurePath = params.get("figure") ?? "/examples/piglet/figure.ts";
  const clipName = params.get("clip") ?? "walk";
  const debug = params.get("debug") !== "0";
  const asset = await loadBrowserFigure(figurePath);
  assertValidFigure(asset);
  const panels = renderVideo(video, asset, { clipName, debug });
  window.assetLabVideoDuration = clipDuration(asset.clips[clipName]) || 1;
  window.assetLabSetVideoTime = (timeSeconds: number) => {
    for (const panel of panels) {
      panel.figureScene.update(timeSeconds);
      panel.figureScene.root.updateMatrixWorld(true);
      panel.floor.update(timeSeconds);
      panel.renderer.render(panel.scene, panel.camera);
    }
  };
  window.assetLabSetVideoTime(0);
  window.assetLabVideoReady = true;
} catch (error) {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  video.innerHTML = `<pre class="error"></pre>`;
  const pre = video.querySelector("pre");
  if (pre) {
    pre.textContent = message;
  }
  window.assetLabVideoReady = true;
}

async function loadBrowserFigure(figurePath: string): Promise<FigureAsset> {
  const module = (await import(/* @vite-ignore */ figurePath)) as { default?: unknown; asset?: unknown };
  const asset = module.default ?? module.asset;
  if (!isFigureAsset(asset)) {
    throw new Error(`Expected '${figurePath}' to export a FigureAsset as default`);
  }
  return asset;
}

function renderVideo(
  container: HTMLElement,
  asset: FigureAsset,
  options: { clipName: string; debug: boolean },
): RenderPanel[] {
  const clip = asset.clips[options.clipName];
  const duration = clipDuration(clip);
  const locomotion = locomotionSummary(clip?.locomotion);
  const subtitle = `${options.clipName} · ${locomotion ?? `${duration.toFixed(2)}s cycle`} · ${
    options.debug ? "debug overlays" : "clean render"
  }`;
  container.innerHTML = `
    <header class="title">
      <h1>${escapeHtml(asset.name)}</h1>
      <span>${escapeHtml(subtitle)}</span>
    </header>
    <section class="views">
      <article class="cell" data-view="right"><div class="viewport"></div><div class="label">Side</div></article>
      <article class="cell" data-view="three-quarter"><div class="viewport"></div><div class="label">Three-quarter</div></article>
    </section>
  `;

  return [
    renderPanel(requiredElement(container, "[data-view='right']"), asset, {
      clipName: options.clipName,
      debug: options.debug,
      view: "right",
    }),
    renderPanel(requiredElement(container, "[data-view='three-quarter']"), asset, {
      clipName: options.clipName,
      debug: options.debug,
      view: "three-quarter",
    }),
  ];
}

function renderPanel(
  cell: HTMLElement,
  asset: FigureAsset,
  options: { clipName: string; debug: boolean; view: ViewName },
): RenderPanel {
  const viewport = requiredElement(cell, ".viewport");
  const width = viewport.clientWidth;
  const height = viewport.clientHeight;

  const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
  renderer.setPixelRatio(1);
  renderer.setSize(width, height);
  renderer.setClearColor(new THREE.Color("#edf1f4"));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  viewport.appendChild(renderer.domElement);

  const scene = createLitScene();
  const clip = asset.clips[options.clipName];
  const figureScene = createFigureScene(asset, options.clipName, {
    debug: options.debug,
    labels: false,
  });
  scene.add(figureScene.root);
  figureScene.update(0);
  figureScene.root.updateMatrixWorld(true);

  const bounds = new THREE.Box3().setFromObject(figureScene.root);
  if (options.debug) {
    scene.add(new THREE.Box3Helper(bounds, new THREE.Color("#0f766e")));
  }
  const center = bounds.getCenter(new THREE.Vector3());
  const size = bounds.getSize(new THREE.Vector3());
  const floor = createReviewFloor(bounds, clip?.locomotion, clipDuration(clip));
  scene.add(floor.root);
  floor.update(0);

  const camera = new THREE.PerspectiveCamera(35, width / height, 0.01, 100);
  placeCamera(camera, center, size, options.view);
  renderer.render(scene, camera);

  return { camera, figureScene, floor, renderer, scene };
}

function createLitScene(): THREE.Scene {
  const scene = new THREE.Scene();
  scene.add(new THREE.HemisphereLight("#ffffff", "#9eb1c1", 2.1));
  const key = new THREE.DirectionalLight("#ffffff", 2.7);
  key.position.set(3, 5, -4);
  scene.add(key);
  const fill = new THREE.DirectionalLight("#ffffff", 0.8);
  fill.position.set(-4, 2, 3);
  scene.add(fill);
  return scene;
}

function placeCamera(camera: THREE.PerspectiveCamera, center: THREE.Vector3, size: THREE.Vector3, view: ViewName): void {
  const directions: Record<ViewName, THREE.Vector3> = {
    right: new THREE.Vector3(1, 0.2, 0),
    "three-quarter": new THREE.Vector3(0.78, 0.34, -1),
  };
  const radius = Math.max(size.length() * 0.5, 0.75);
  const fov = THREE.MathUtils.degToRad(camera.fov);
  const distance = Math.max(2.4, (radius / Math.sin(fov / 2)) * 1.3);
  const direction = directions[view].normalize();
  camera.position.copy(center.clone().add(direction.multiplyScalar(distance)));
  camera.lookAt(center);
}

function requiredElement(parent: ParentNode, selector: string): HTMLElement {
  const element = parent.querySelector<HTMLElement>(selector);
  if (!element) {
    throw new Error(`Missing required element '${selector}'`);
  }
  return element;
}

function isFigureAsset(value: unknown): value is FigureAsset {
  if (!value || typeof value !== "object") {
    return false;
  }
  const asset = value as Partial<FigureAsset>;
  return asset.schemaVersion === 1 && typeof asset.name === "string" && Array.isArray(asset.parts);
}

function escapeHtml(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}
