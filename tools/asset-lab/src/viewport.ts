import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { ClipSpec, FigureAsset } from "./dsl";
import { createReviewFloor, type ReviewFloor } from "./review-floor";
import { clipDuration, createFigureScene, disposeObjectTree, type FigureScene } from "./scene";

export type CameraPreset =
  | "front"
  | "right"
  | "back"
  | "left"
  | "top"
  | "three-quarter";

export interface FigureViewportOptions {
  background?: string;
  onClipComplete?: (clipName: string) => void;
  onTimeChange?: (timeSeconds: number) => void;
  pixelRatioCap?: number;
  showFloor?: boolean;
}

export class FigureViewportController {
  readonly canvas: HTMLCanvasElement;

  private readonly camera: THREE.PerspectiveCamera;
  private readonly controls: OrbitControls;
  private readonly renderer: THREE.WebGLRenderer;
  private readonly resizeObserver: ResizeObserver;
  private readonly scene = new THREE.Scene();
  private readonly onTimeChange: ((timeSeconds: number) => void) | undefined;
  private readonly onClipComplete: ((clipName: string) => void) | undefined;
  private readonly showFloor: boolean;
  private animationFrame = 0;
  private asset: FigureAsset | undefined;
  private bounds: THREE.Box3 | undefined;
  private clip: ClipSpec | undefined;
  private clipName: string | undefined;
  private destroyed = false;
  private durationSeconds = 0;
  private figure: FigureScene | undefined;
  private floor: ReviewFloor | undefined;
  private lastFrameMilliseconds: number | undefined;
  private lastTimeNotificationMilliseconds = Number.NEGATIVE_INFINITY;
  private playbackSpeed = 1;
  private playing = true;
  private timeSeconds = 0;

  constructor(private readonly container: HTMLElement, options: FigureViewportOptions = {}) {
    this.onClipComplete = options.onClipComplete;
    this.onTimeChange = options.onTimeChange;
    this.showFloor = options.showFloor ?? true;
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.setClearColor(new THREE.Color(options.background ?? "#edf1f4"));
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, options.pixelRatioCap ?? 2));
    this.canvas = this.renderer.domElement;
    this.canvas.className = "figureViewportCanvas";
    this.canvas.dataset.cameraPreset = "three-quarter";
    this.canvas.dataset.figure = "";
    this.canvas.dataset.clip = "";
    this.canvas.tabIndex = 0;
    this.canvas.setAttribute(
      "aria-label",
      "Interactive 3D figure. Drag to orbit, right-drag to pan, and scroll to zoom.",
    );
    container.appendChild(this.canvas);

    this.camera = new THREE.PerspectiveCamera(38, 1, 0.01, 100);
    this.controls = new OrbitControls(this.camera, this.canvas);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.075;
    this.controls.screenSpacePanning = true;
    this.controls.addEventListener("start", () => {
      this.canvas.dataset.cameraPreset = "custom";
    });

    this.scene.add(new THREE.HemisphereLight("#ffffff", "#8ca0ae", 2.15));
    const key = new THREE.DirectionalLight("#ffffff", 2.65);
    key.position.set(3, 5, -4);
    this.scene.add(key);
    const fill = new THREE.DirectionalLight("#ffffff", 0.75);
    fill.position.set(-4, 2, 3);
    this.scene.add(fill);

    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(container);
    this.resize();
    this.animationFrame = requestAnimationFrame(this.renderFrame);
  }

  setAsset(asset: FigureAsset, clipName?: string): void {
    this.removeFigure();
    this.asset = asset;
    this.figure = createFigureScene(asset, clipName, {
      debug: false,
      jointMarkers: false,
      labels: false,
    });
    this.figure.update(0);
    this.figure.root.updateMatrixWorld(true);
    this.scene.add(this.figure.root);
    this.bounds = new THREE.Box3().setFromObject(this.figure.root);
    assertUsableBounds(this.bounds, asset.name);
    this.canvas.dataset.figure = asset.name;
    this.timeSeconds = 0;
    this.configureClip(clipName);
    this.setCameraPreset("three-quarter");
    this.notifyTime(true);
  }

  setClip(clipName?: string): void {
    if (!this.asset || !this.figure) {
      return;
    }
    this.figure.setClip(clipName);
    this.timeSeconds = 0;
    this.configureClip(clipName);
    this.notifyTime(true);
  }

  setPlaying(playing: boolean): void {
    this.playing = playing && this.durationSeconds > 0;
    this.lastFrameMilliseconds = undefined;
    this.canvas.dataset.playing = String(this.playing);
  }

  isPlaying(): boolean {
    return this.playing;
  }

  setPlaybackSpeed(speed: number): void {
    if (!Number.isFinite(speed) || speed <= 0) {
      throw new Error(`Playback speed must be positive, got '${speed}'`);
    }
    this.playbackSpeed = speed;
    this.canvas.dataset.playbackSpeed = String(speed);
  }

  setTime(timeSeconds: number): void {
    if (!Number.isFinite(timeSeconds) || timeSeconds < 0) {
      throw new Error(`Animation time must be finite and nonnegative, got '${timeSeconds}'`);
    }
    this.timeSeconds = Math.min(timeSeconds, this.durationSeconds);
    this.updatePose();
    this.notifyTime(true);
  }

  getTime(): number {
    return this.timeSeconds;
  }

  getDuration(): number {
    return this.durationSeconds;
  }

  getClipName(): string | undefined {
    return this.clipName;
  }

  fit(): void {
    if (!this.bounds) {
      return;
    }
    const direction = this.camera.position.clone().sub(this.controls.target);
    if (direction.lengthSq() < 1e-8) {
      direction.copy(cameraDirection("three-quarter"));
    }
    this.positionCamera(direction.normalize(), "custom");
  }

  setCameraPreset(preset: CameraPreset): void {
    if (!this.bounds) {
      return;
    }
    this.positionCamera(cameraDirection(preset), preset);
  }

  setBackground(color: string): void {
    this.renderer.setClearColor(new THREE.Color(color));
  }

  dispose(): void {
    if (this.destroyed) {
      return;
    }
    this.destroyed = true;
    cancelAnimationFrame(this.animationFrame);
    this.resizeObserver.disconnect();
    this.removeFigure();
    this.controls.dispose();
    this.renderer.renderLists.dispose();
    this.renderer.dispose();
    this.renderer.forceContextLoss();
    this.canvas.remove();
  }

  private configureClip(clipName?: string): void {
    this.clipName = clipName;
    this.clip = clipName === undefined ? undefined : this.asset?.clips[clipName];
    this.durationSeconds = clipDuration(this.clip);
    this.canvas.dataset.clip = clipName ?? "";
    this.canvas.dataset.duration = String(this.durationSeconds);
    this.replaceFloor();
    this.setPlaying(this.durationSeconds > 0);
    this.updatePose();
  }

  private readonly renderFrame = (milliseconds: number): void => {
    if (this.destroyed) {
      return;
    }
    const previous = this.lastFrameMilliseconds;
    this.lastFrameMilliseconds = milliseconds;
    if (this.playing && previous !== undefined && this.durationSeconds > 0) {
      const elapsed = Math.min((milliseconds - previous) / 1000, 0.1) * this.playbackSpeed;
      const nextTime = this.timeSeconds + elapsed;
      let completedClip: string | undefined;
      if (this.clip?.loop) {
        this.timeSeconds = nextTime % this.durationSeconds;
      } else if (nextTime >= this.durationSeconds) {
        this.timeSeconds = this.durationSeconds;
        completedClip = this.clipName;
        this.setPlaying(false);
      } else {
        this.timeSeconds = nextTime;
      }
      this.updatePose();
      this.notifyTime(false, milliseconds);
      if (completedClip !== undefined) {
        this.onClipComplete?.(completedClip);
      }
    }

    this.controls.update();
    this.renderer.render(this.scene, this.camera);
    this.animationFrame = requestAnimationFrame(this.renderFrame);
  };

  private updatePose(): void {
    this.figure?.update(this.timeSeconds);
    this.floor?.update(this.timeSeconds);
    this.canvas.dataset.time = this.timeSeconds.toFixed(4);
  }

  private notifyTime(force: boolean, milliseconds = performance.now()): void {
    if (!this.onTimeChange) {
      return;
    }
    if (!force && milliseconds - this.lastTimeNotificationMilliseconds < 50) {
      return;
    }
    this.lastTimeNotificationMilliseconds = milliseconds;
    this.onTimeChange(this.timeSeconds);
  }

  private positionCamera(direction: THREE.Vector3, preset: CameraPreset | "custom"): void {
    const bounds = this.bounds;
    if (!bounds) {
      return;
    }
    const sphere = bounds.getBoundingSphere(new THREE.Sphere());
    const radius = Math.max(sphere.radius, 0.1);
    const verticalFov = THREE.MathUtils.degToRad(this.camera.fov);
    const horizontalFov = 2 * Math.atan(Math.tan(verticalFov / 2) * Math.max(this.camera.aspect, 0.1));
    const limitingFov = Math.min(verticalFov, horizontalFov);
    const distance = (radius / Math.sin(limitingFov / 2)) * 1.18;
    const center = bounds.getCenter(new THREE.Vector3());
    this.controls.target.copy(center);
    this.camera.position.copy(center).add(direction.clone().normalize().multiplyScalar(distance));
    this.camera.near = Math.max(distance / 1000, 0.001);
    this.camera.far = Math.max(distance * 20, 100);
    this.camera.updateProjectionMatrix();
    this.camera.lookAt(center);
    this.controls.update();
    this.canvas.dataset.cameraPreset = preset;
  }

  private resize(): void {
    const width = Math.max(1, this.container.clientWidth);
    const height = Math.max(1, this.container.clientHeight);
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
  }

  private replaceFloor(): void {
    if (this.floor) {
      this.scene.remove(this.floor.root);
      disposeObjectTree(this.floor.root);
      this.floor = undefined;
    }
    if (!this.showFloor || !this.bounds) {
      return;
    }
    this.floor = createReviewFloor(this.bounds, this.clip?.locomotion, this.durationSeconds);
    this.scene.add(this.floor.root);
  }

  private removeFigure(): void {
    if (this.floor) {
      this.scene.remove(this.floor.root);
      disposeObjectTree(this.floor.root);
      this.floor = undefined;
    }
    if (this.figure) {
      this.scene.remove(this.figure.root);
      this.figure.dispose();
      this.figure = undefined;
    }
    this.asset = undefined;
    this.bounds = undefined;
    this.clip = undefined;
    this.clipName = undefined;
    this.durationSeconds = 0;
    this.timeSeconds = 0;
    this.canvas.dataset.figure = "";
    this.canvas.dataset.clip = "";
  }
}

function assertUsableBounds(bounds: THREE.Box3, figureName: string): void {
  const size = bounds.getSize(new THREE.Vector3());
  if (
    bounds.isEmpty()
    || !Number.isFinite(size.x)
    || !Number.isFinite(size.y)
    || !Number.isFinite(size.z)
    || size.lengthSq() <= 0
  ) {
    throw new Error(`Figure '${figureName}' has invalid display bounds`);
  }
}

function cameraDirection(preset: CameraPreset): THREE.Vector3 {
  switch (preset) {
    case "front":
      return new THREE.Vector3(0, 0.12, -1).normalize();
    case "right":
      return new THREE.Vector3(1, 0.12, 0).normalize();
    case "back":
      return new THREE.Vector3(0, 0.12, 1).normalize();
    case "left":
      return new THREE.Vector3(-1, 0.12, 0).normalize();
    case "top":
      return new THREE.Vector3(0, 1, 0.001).normalize();
    case "three-quarter":
      return new THREE.Vector3(0.78, 0.34, -1).normalize();
  }
}
