import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { assertValidFigure, type FigureAsset } from "./dsl";
import { createFigureScene } from "./scene";

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
  const figurePath = params.get("figure") ?? "/examples/piglet/figure.ts";
  const clipName = params.get("clip") ?? "walk";
  const asset = await loadBrowserFigure(figurePath);
  assertValidFigure(asset);
  startPreview(app, asset, clipName);
} catch (error) {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  app.innerHTML = `<pre class="error"></pre>`;
  const pre = app.querySelector("pre");
  if (pre) {
    pre.textContent = message;
  }
  window.assetLabReady = true;
}

async function loadBrowserFigure(figurePath: string): Promise<FigureAsset> {
  const module = (await import(/* @vite-ignore */ figurePath)) as { default?: unknown; asset?: unknown };
  const asset = module.default ?? module.asset;
  if (!isFigureAsset(asset)) {
    throw new Error(`Expected '${figurePath}' to export a FigureAsset as default`);
  }
  return asset;
}

function startPreview(container: HTMLElement, asset: FigureAsset, clipName?: string): void {
  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setSize(container.clientWidth, container.clientHeight);
  renderer.setClearColor(new THREE.Color("#edf1f4"));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  container.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(45, container.clientWidth / container.clientHeight, 0.01, 100);
  camera.position.set(2.5, 1.4, -2.7);
  camera.lookAt(0, 0, 0);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.set(0, 0, 0);
  controls.enableDamping = true;

  scene.add(new THREE.HemisphereLight("#ffffff", "#9eb1c1", 2.1));
  const key = new THREE.DirectionalLight("#ffffff", 2.6);
  key.position.set(3, 5, 4);
  scene.add(key);

  const grid = new THREE.GridHelper(4, 16, "#8a98a6", "#c5cdd5");
  grid.position.y = -0.86;
  scene.add(grid);

  const figureScene = createFigureScene(asset, clipName && asset.clips[clipName] ? clipName : undefined);
  scene.add(figureScene.root);

  const bounds = new THREE.Box3().setFromObject(figureScene.root);
  const center = bounds.getCenter(new THREE.Vector3());
  controls.target.copy(center);
  controls.update();

  const clock = new THREE.Clock();

  function render(): void {
    const elapsed = clock.getElapsedTime();
    figureScene.update(elapsed);
    controls.update();
    renderer.render(scene, camera);
    window.assetLabReady = true;
    requestAnimationFrame(render);
  }

  render();

  window.addEventListener("resize", () => {
    const width = container.clientWidth;
    const height = container.clientHeight;
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height);
  });
}

function isFigureAsset(value: unknown): value is FigureAsset {
  if (!value || typeof value !== "object") {
    return false;
  }
  const asset = value as Partial<FigureAsset>;
  return asset.schemaVersion === 1 && typeof asset.name === "string" && Array.isArray(asset.parts);
}
