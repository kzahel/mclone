// Debug tooling — see tactical 40. Throwaway when real Player/Input lands.

import { SectionPos } from "../../core/section-pos";
import { Vec3 } from "../../world/phys/vec3";
import {
  createSceneDepthView,
  encodeSceneFrame,
  initializeRendererScene,
  type RendererScene,
} from "../scene-setup";
import { DebugInput } from "./debug-input";

const SEED = 12_345n;
const VIEW_DISTANCE = 2;
const SPEED_BASE = 20.0;
const SPEED_BOOST = 4.0;
const MOUSE_SENS_DEG_PER_PIXEL = 0.15;
const JOYSTICK_LOOK_DEG_PER_SEC = 120.0;
const MAX_PITCH = 89.0;
const LIGHT_TICK_INTERVAL_MS = 1000.0;

const INITIAL_POSITION = new Vec3(8.5, 104.0, 40.5);
const INITIAL_X_ROT = 30.0;
const INITIAL_Y_ROT = 180.0;

interface CameraState {
  position: Vec3;
  xRot: number;
  yRot: number;
}

function buildCameraDeltaWorld(
  yawDeg: number,
  pitchDeg: number,
  forwardAxis: number,
  rightAxis: number,
  upAxis: number,
): { x: number; y: number; z: number } {
  const yawRad = (yawDeg * Math.PI) / 180.0;
  const pitchRad = (pitchDeg * Math.PI) / 180.0;
  const sinYaw = Math.sin(yawRad);
  const cosYaw = Math.cos(yawRad);
  const sinPitch = Math.sin(pitchRad);
  const cosPitch = Math.cos(pitchRad);

  const forwardX = -sinYaw * cosPitch;
  const forwardY = -sinPitch;
  const forwardZ = cosYaw * cosPitch;
  const rightX = -cosYaw;
  const rightZ = -sinYaw;

  return {
    x: forwardX * forwardAxis + rightX * rightAxis,
    y: forwardY * forwardAxis + upAxis,
    z: forwardZ * forwardAxis + rightZ * rightAxis,
  };
}

function applyInputToCamera(
  camera: CameraState,
  frame: import("./debug-input").DebugInputFrame,
  dtSeconds: number,
): CameraState {
  const yawDelta =
    frame.mouseDeltaX * MOUSE_SENS_DEG_PER_PIXEL
    + frame.joystickX * JOYSTICK_LOOK_DEG_PER_SEC * dtSeconds;
  const pitchDelta =
    frame.mouseDeltaY * MOUSE_SENS_DEG_PER_PIXEL
    - frame.joystickY * JOYSTICK_LOOK_DEG_PER_SEC * dtSeconds;

  let xRot = camera.xRot + pitchDelta;
  if (xRot > MAX_PITCH) xRot = MAX_PITCH;
  if (xRot < -MAX_PITCH) xRot = -MAX_PITCH;
  const yRot = camera.yRot + yawDelta;

  const forwardAxis =
    (frame.heldKeys.has("KeyW") || frame.moveForward ? 1 : 0)
    - (frame.heldKeys.has("KeyS") || frame.moveBack ? 1 : 0);
  const rightAxis =
    (frame.heldKeys.has("KeyD") ? 1 : 0) - (frame.heldKeys.has("KeyA") ? 1 : 0);
  const upAxis =
    (frame.heldKeys.has("Space") || frame.flyUp ? 1 : 0)
    - (frame.heldKeys.has("ShiftLeft") || frame.heldKeys.has("ShiftRight") || frame.flyDown ? 1 : 0);

  if (forwardAxis === 0 && rightAxis === 0 && upAxis === 0) {
    return { position: camera.position, xRot, yRot };
  }

  const delta = buildCameraDeltaWorld(yRot, xRot, forwardAxis, rightAxis, upAxis);
  const length = Math.hypot(delta.x, delta.y, delta.z);
  if (length === 0) {
    return { position: camera.position, xRot, yRot };
  }

  const boost = frame.heldKeys.has("ControlLeft") || frame.heldKeys.has("ControlRight") ? SPEED_BOOST : 1.0;
  const speed = (SPEED_BASE * boost * dtSeconds) / length;
  const position = new Vec3(
    camera.position.x + delta.x * speed,
    camera.position.y + delta.y * speed,
    camera.position.z + delta.z * speed,
  );
  return { position, xRot, yRot };
}

function showOverlayMessage(message: string): void {
  const overlay = document.querySelector<HTMLElement>("#debug-overlay");
  if (overlay) overlay.textContent = message;
}

async function boot(): Promise<void> {
  const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
  if (!canvas) {
    showOverlayMessage("error: canvas #renderer not found");
    return;
  }

  const sceneResult = await initializeRendererScene(canvas, { seed: SEED, viewDistance: VIEW_DISTANCE });
  if (!sceneResult.ok) {
    showOverlayMessage(`error: ${sceneResult.reason}`);
    return;
  }
  const scene: RendererScene = sceneResult.scene;

  const input = new DebugInput(canvas);
  const depthView = createSceneDepthView(scene.device, canvas.width, canvas.height);

  let camera: CameraState = {
    position: INITIAL_POSITION,
    xRot: INITIAL_X_ROT,
    yRot: INITIAL_Y_ROT,
  };
  let lastFrameMs = performance.now();
  let lastLightTickMs = lastFrameMs;
  let renderInFlight = false;
  let frameCount = 0;
  let lastFpsReportMs = lastFrameMs;

  // Prime chunks at the start so the first frame has something to draw.
  if (await scene.worldClient.setChunkView({
    type: "set_chunk_view",
    centerChunkX: SectionPos.posToSectionCoord(camera.position.x),
    centerChunkZ: SectionPos.posToSectionCoord(camera.position.z),
    radius: scene.viewDistance,
  })) {
    scene.levelRenderer.allChanged();
  }

  const isTouch = typeof window !== "undefined" && window.matchMedia("(pointer: coarse)").matches;
  showOverlayMessage(
    isTouch
      ? "joystick: look · FWD/BACK: move · ▲/▼: fly"
      : "click to capture mouse — WASD + mouse, Space/Shift for up/down, Ctrl to boost, Esc to release",
  );

  async function tick(): Promise<void> {
    const now = performance.now();
    const dtSeconds = Math.min(0.1, (now - lastFrameMs) / 1000.0);
    lastFrameMs = now;

    const inputFrame = input.consumeFrame();
    if (inputFrame.locked) {
      camera = applyInputToCamera(camera, inputFrame, dtSeconds);
    }

    if (now - lastLightTickMs >= LIGHT_TICK_INTERVAL_MS) {
      scene.lightTexture.tick();
      lastLightTickMs = now;
    }

    if (await scene.worldClient.setChunkView({
      type: "set_chunk_view",
      centerChunkX: SectionPos.posToSectionCoord(camera.position.x),
      centerChunkZ: SectionPos.posToSectionCoord(camera.position.z),
      radius: scene.viewDistance,
    })) {
      scene.levelRenderer.allChanged();
    }

    if (!renderInFlight) {
      renderInFlight = true;
      try {
        const renderedFrame = await scene.gameRenderer.renderLevel(
          0.0,
          Number.MAX_SAFE_INTEGER,
          scene.levelRenderer,
          scene.lightTexture,
          camera,
        );
        const encoder = scene.device.createCommandEncoder();
        encodeSceneFrame(
          scene,
          renderedFrame,
          {
            view: scene.ctx.getCurrentTexture().createView(),
            depthView,
            format: scene.format,
          },
          encoder,
        );
        scene.device.queue.submit([encoder.finish()]);
        frameCount++;
        if (now - lastFpsReportMs >= 1000.0) {
          const fps = (frameCount * 1000.0) / (now - lastFpsReportMs);
          if (inputFrame.locked) {
            showOverlayMessage(
              `fps ${fps.toFixed(0)}  pos ${camera.position.x.toFixed(1)}, ${camera.position.y.toFixed(1)}, ${camera.position.z.toFixed(1)}  yaw ${camera.yRot.toFixed(0)}  pitch ${camera.xRot.toFixed(0)}`,
            );
          }
          frameCount = 0;
          lastFpsReportMs = now;
        }
      } finally {
        renderInFlight = false;
      }
    }

    requestAnimationFrame(() => {
      void tick();
    });
  }

  requestAnimationFrame(() => {
    void tick();
  });
}

if (typeof window !== "undefined") {
  boot().catch((err: unknown) => {
    const message = err instanceof Error ? `${err.name}: ${err.message}` : String(err);
    showOverlayMessage(`error: ${message}`);
    // eslint-disable-next-line no-console
    console.error(err);
  });
  window.addEventListener("error", (ev) => {
    showOverlayMessage(`error: ${ev.message}`);
  });
  window.addEventListener("unhandledrejection", (ev) => {
    const reason = ev.reason instanceof Error ? ev.reason.message : String(ev.reason);
    showOverlayMessage(`error: ${reason}`);
  });
}
