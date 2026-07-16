import * as THREE from "three";
import { loadBrowserFigure } from "./browser-load";
import { clipDuration, createFigureScene } from "./scene";

export interface FigureReviewContract {
  panelWidth: number;
  panelHeight: number;
  fovDegrees: number;
  distance: number;
  target: [number, number, number];
  background: "#edf1f4";
  animation?: FigureAnimationReviewContract;
}

export interface FigureAnimationReviewContract {
  clip: string;
  view: "three-quarter";
  durationSeconds: number;
  sampleTimesSeconds: number[];
  captureFramesPerSecond: number;
  captureCycleCount: number;
  captureFrameCount: number;
}

declare global {
  interface Window {
    assetLabReviewContract?: FigureReviewContract;
    assetLabReviewFigureName?: string;
    assetLabReviewReady?: boolean;
    assetLabSetReviewTime?: (timeSeconds: number) => void;
  }
}

type ReviewViewName = "front" | "right" | "three-quarter";

const review = document.querySelector<HTMLElement>("#review");
if (!review) {
  throw new Error("Missing #review");
}

try {
  const params = new URLSearchParams(window.location.search);
  const figurePath = params.get("figure") ?? "/examples/player/figure.ts";
  const panelWidth = parseDimension(params.get("width"), 360);
  const panelHeight = parseDimension(params.get("height"), 480);
  const clipName = params.get("clip") ?? undefined;
  const sampleTimes = parseSampleTimes(params.get("sampleTimes"));
  const captureFramesPerSecond = parsePositiveNumber(params.get("captureFps"), 60);
  const captureCycleCount = parsePositiveNumber(params.get("captureCycles"), 2);
  const asset = await loadBrowserFigure(figurePath);
  const figure = createFigureScene(asset, clipName, {
    debug: false,
    jointMarkers: false,
    labels: false,
  });
  figure.update(0);
  figure.root.updateMatrixWorld(true);

  const bounds = new THREE.Box3().setFromObject(figure.root);
  const size = bounds.getSize(new THREE.Vector3());
  const center = bounds.getCenter(new THREE.Vector3());
  const height = size.y;
  if (!Number.isFinite(height) || height <= 0) {
    throw new Error(`Figure '${asset.name}' has invalid semantic height ${height}`);
  }
  const normalizedSize = size.clone().multiplyScalar(1 / height);
  const radius = Math.max(normalizedSize.length() * 0.5, 0.75);
  const fovDegrees = 35;
  const distance = Math.max(2.2, (radius / Math.sin(THREE.MathUtils.degToRad(fovDegrees) / 2)) * 1.12);
  const contract: FigureReviewContract = {
    panelWidth,
    panelHeight,
    fovDegrees,
    distance,
    target: [0, 0.5, 0],
    background: "#edf1f4",
  };
  if (clipName) {
    const durationSeconds = clipDuration(asset.clips[clipName]);
    if (durationSeconds <= 0) {
      throw new Error(`Figure '${asset.name}' clip '${clipName}' has no positive duration`);
    }
    contract.animation = {
      clip: clipName,
      view: "three-quarter",
      durationSeconds,
      sampleTimesSeconds: sampleTimes,
      captureFramesPerSecond,
      captureCycleCount,
      captureFrameCount: Math.max(
        2,
        Math.ceil(durationSeconds * captureCycleCount * captureFramesPerSecond),
      ),
    };
  }

  const scene = createLitScene();
  scene.add(figure.root);
  const panels: Array<{
    camera: THREE.PerspectiveCamera;
    renderer: THREE.WebGLRenderer;
  }> = [];
  for (const [viewName, direction] of reviewViews()) {
    const viewport = document.createElement("div");
    viewport.className = "viewport";
    viewport.dataset.view = viewName;
    viewport.style.width = `${panelWidth}px`;
    viewport.style.height = `${panelHeight}px`;
    review.appendChild(viewport);

    const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
    renderer.setPixelRatio(1);
    renderer.setSize(panelWidth, panelHeight);
    renderer.setClearColor(new THREE.Color(contract.background));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    viewport.appendChild(renderer.domElement);

    const camera = new THREE.PerspectiveCamera(
      fovDegrees,
      panelWidth / panelHeight,
      0.01 * height,
      100 * height,
    );
    camera.position.copy(center.clone().add(direction.normalize().multiplyScalar(distance * height)));
    camera.lookAt(center);
    renderer.render(scene, camera);
    panels.push({ camera, renderer });
  }

  window.assetLabSetReviewTime = (timeSeconds: number) => {
    figure.update(timeSeconds);
    figure.root.updateMatrixWorld(true);
    for (const panel of panels) {
      panel.renderer.render(scene, panel.camera);
    }
  };

  window.assetLabReviewContract = contract;
  window.assetLabReviewFigureName = asset.name;
  window.assetLabReviewReady = true;
} catch (error) {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  review.innerHTML = `<pre class="error"></pre>`;
  const pre = review.querySelector("pre");
  if (pre) {
    pre.textContent = message;
  }
  window.assetLabReviewReady = true;
}

function reviewViews(): Array<[ReviewViewName, THREE.Vector3]> {
  return [
    ["front", new THREE.Vector3(0, 0.2, -1)],
    ["right", new THREE.Vector3(1, 0.2, 0)],
    ["three-quarter", new THREE.Vector3(0.78, 0.34, -1)],
  ];
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

function parseDimension(value: string | null, fallback: number): number {
  const parsed = value === null ? fallback : Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0 || parsed > 4096) {
    throw new Error(`Review dimensions must be integers from 1 through 4096, got '${value}'`);
  }
  return parsed;
}

function parsePositiveNumber(value: string | null, fallback: number): number {
  const parsed = value === null ? fallback : Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`Animation capture values must be positive, got '${value}'`);
  }
  return parsed;
}

function parseSampleTimes(value: string | null): number[] {
  const fallback = [0, 0.045, 0.09, 0.123, 0.125, 0.45, 0.899, 0.901];
  const parsed = value === null ? fallback : value.split(",").map(Number);
  if (parsed.length === 0 || parsed.some((time) => !Number.isFinite(time) || time < 0)) {
    throw new Error(`Animation sample times must be finite and nonnegative, got '${value}'`);
  }
  return parsed;
}
