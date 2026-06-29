import * as THREE from "three";
import { assertValidFigure, type FigureAsset } from "./dsl";
import { clipDuration, createFigureScene } from "./scene";

declare global {
  interface Window {
    assetLabSheetReady?: boolean;
  }
}

type ViewName = "front" | "right" | "three-quarter";

interface ViewSpec {
  label: string;
  view: ViewName;
  time: number;
}

const sheet = document.querySelector<HTMLElement>("#sheet");
if (!sheet) {
  throw new Error("Missing #sheet");
}

try {
  const params = new URLSearchParams(window.location.search);
  const figurePath = params.get("figure") ?? "/examples/piglet/figure.ts";
  const clipName = params.get("clip") ?? "walk";
  const debug = params.get("debug") !== "0";
  const labels = params.get("labels") === "1";
  const asset = await loadBrowserFigure(figurePath);
  assertValidFigure(asset);
  renderSheet(sheet, asset, { clipName, debug, labels });
  window.assetLabSheetReady = true;
} catch (error) {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  sheet.innerHTML = `<pre class="error"></pre>`;
  const pre = sheet.querySelector("pre");
  if (pre) {
    pre.textContent = message;
  }
  window.assetLabSheetReady = true;
}

async function loadBrowserFigure(figurePath: string): Promise<FigureAsset> {
  const module = (await import(/* @vite-ignore */ figurePath)) as { default?: unknown; asset?: unknown };
  const asset = module.default ?? module.asset;
  if (!isFigureAsset(asset)) {
    throw new Error(`Expected '${figurePath}' to export a FigureAsset as default`);
  }
  return asset;
}

function renderSheet(
  container: HTMLElement,
  asset: FigureAsset,
  options: { clipName: string; debug: boolean; labels: boolean },
): void {
  const clip = asset.clips[options.clipName];
  const duration = clipDuration(clip);
  const frameCount = 6;
  const frameTimes =
    duration > 0 ? Array.from({ length: frameCount }, (_, index) => (duration * index) / frameCount) : [0];

  container.innerHTML = `
    <header class="title">
      <h1>${escapeHtml(asset.name)}</h1>
      <span>${escapeHtml(options.clipName)} · ${options.debug ? "debug overlays" : "clean render"}</span>
    </header>
    <div class="section-title">Static Views</div>
    <section class="grid static-grid" data-section="static"></section>
    <div class="section-title">Side Animation Strip</div>
    <section class="grid strip-grid" data-section="strip-side"></section>
    <div class="section-title">Three-Quarter Animation Strip</div>
    <section class="grid strip-grid" data-section="strip-three-quarter"></section>
  `;

  const staticSection = requiredElement(container, "[data-section='static']");
  const sideStripSection = requiredElement(container, "[data-section='strip-side']");
  const threeQuarterStripSection = requiredElement(container, "[data-section='strip-three-quarter']");

  const staticViews: ViewSpec[] = [
    { label: "Front", view: "front", time: 0 },
    { label: "Right", view: "right", time: 0 },
    { label: "Three-quarter", view: "three-quarter", time: 0 },
  ];

  for (const spec of staticViews) {
    renderCell(staticSection, asset, {
      label: spec.label,
      view: spec.view,
      clipName: options.clipName,
      time: spec.time,
      debug: options.debug,
      labels: options.labels,
    });
  }

  renderStrip(sideStripSection, asset, {
    clipName: options.clipName,
    debug: options.debug,
    frameTimes,
    labels: false,
    labelPrefix: "side",
    view: "right",
  });
  renderStrip(threeQuarterStripSection, asset, {
    clipName: options.clipName,
    debug: options.debug,
    frameTimes,
    labels: false,
    labelPrefix: "3q",
    view: "three-quarter",
  });
}

function renderStrip(
  parent: HTMLElement,
  asset: FigureAsset,
  options: {
    clipName: string;
    debug: boolean;
    frameTimes: number[];
    labelPrefix: string;
    labels: boolean;
    view: ViewName;
  },
): void {
  for (const [index, time] of options.frameTimes.entries()) {
    renderCell(parent, asset, {
      label: `${options.clipName} ${options.labelPrefix} ${index + 1}/${options.frameTimes.length}`,
      view: options.view,
      clipName: options.clipName,
      time,
      debug: options.debug,
      labels: options.labels,
    });
  }
}

function renderCell(
  parent: HTMLElement,
  asset: FigureAsset,
  options: {
    label: string;
    view: ViewName;
    clipName: string;
    time: number;
    debug: boolean;
    labels: boolean;
  },
): void {
  const cell = document.createElement("article");
  cell.className = "cell";
  cell.innerHTML = `<div class="viewport"></div><div class="label">${escapeHtml(options.label)}</div>`;
  parent.appendChild(cell);

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
  const figureScene = createFigureScene(asset, options.clipName, {
    debug: options.debug,
    labels: options.labels,
  });
  scene.add(figureScene.root);
  figureScene.update(options.time);
  figureScene.root.updateMatrixWorld(true);

  const bounds = new THREE.Box3().setFromObject(figureScene.root);
  if (options.debug) {
    scene.add(new THREE.Box3Helper(bounds, new THREE.Color("#0f766e")));
  }
  const center = bounds.getCenter(new THREE.Vector3());
  const size = bounds.getSize(new THREE.Vector3());
  const floor = new THREE.GridHelper(Math.max(3, Math.ceil(Math.max(size.x, size.z) * 3)), 12, "#8a98a6", "#c5cdd5");
  floor.position.y = bounds.min.y - 0.035;
  scene.add(floor);

  const camera = new THREE.PerspectiveCamera(35, width / height, 0.01, 100);
  placeCamera(camera, center, size, options.view);
  renderer.render(scene, camera);
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
    front: new THREE.Vector3(0, 0.2, -1),
    right: new THREE.Vector3(1, 0.2, 0),
    "three-quarter": new THREE.Vector3(0.78, 0.34, -1),
  };
  const radius = Math.max(size.length() * 0.5, 0.75);
  const fov = THREE.MathUtils.degToRad(camera.fov);
  const distance = Math.max(2.2, (radius / Math.sin(fov / 2)) * 1.12);
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
