import type { GuiOverlayHost } from "./gui/gui-overlay-host";
import {
  createGeneratedWorldSmokePresentationHost,
  type GeneratedWorldSmokePresentationHost,
} from "./generated-world-smoke-runner";
import {
  createSceneDepthTarget,
  encodeSceneFrame,
} from "./scene-setup";
import { readTextureRgba8 } from "./webgpu-target";

export interface GeneratedWorldBrowserPresentationHostOptions {
  readonly canvas: HTMLCanvasElement;
  readonly readbackFormat: GPUTextureFormat;
  readonly guiProbeOverlay?: GuiOverlayHost;
}

export function createGeneratedWorldBrowserPresentationHost(
  options: GeneratedWorldBrowserPresentationHostOptions,
): GeneratedWorldSmokePresentationHost {
  return createGeneratedWorldSmokePresentationHost({
    target: {
      get width() {
        return options.canvas.width;
      },
      get height() {
        return options.canvas.height;
      },
      format: options.readbackFormat,
      renderFrame: async ({ scene, frame }) => {
        const canvasDepthTarget = createSceneDepthTarget(scene.device, options.canvas.width, options.canvas.height);
        const readbackDepthTarget = createSceneDepthTarget(scene.device, options.canvas.width, options.canvas.height);
        const canvasTexture = scene.ctx.getCurrentTexture();
        const canvasWorldView = canvasTexture.createView();
        const canvasGuiView = canvasTexture.createView();
        const readbackTexture = scene.device.createTexture({
          size: { width: options.canvas.width, height: options.canvas.height },
          format: options.readbackFormat,
          usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
        });
        try {
          const encoder = scene.device.createCommandEncoder();
          encodeSceneFrame(
            scene,
            frame,
            {
              view: canvasWorldView,
              depthView: canvasDepthTarget.view,
              format: scene.format,
            },
            encoder,
          );
          encodeGuiProbeOverlay(options.guiProbeOverlay, encoder, canvasGuiView, scene.format, options.canvas.width, options.canvas.height);
          const readbackWorldView = readbackTexture.createView();
          const readbackGuiView = readbackTexture.createView();
          encodeSceneFrame(
            scene,
            frame,
            {
              view: readbackWorldView,
              depthView: readbackDepthTarget.view,
              format: options.readbackFormat,
            },
            encoder,
          );
          encodeGuiProbeOverlay(options.guiProbeOverlay, encoder, readbackGuiView, options.readbackFormat, options.canvas.width, options.canvas.height);
          scene.device.queue.submit([encoder.finish()]);
          await scene.device.queue.onSubmittedWorkDone();
          return {
            width: options.canvas.width,
            height: options.canvas.height,
            format: options.readbackFormat,
            pixels: await readTextureRgba8(scene.device, readbackTexture, options.canvas.width, options.canvas.height),
          };
        } finally {
          canvasDepthTarget.texture.destroy();
          readbackDepthTarget.texture.destroy();
          readbackTexture.destroy();
        }
      },
    },
  });
}

function encodeGuiProbeOverlay(
  overlay: GuiOverlayHost | undefined,
  encoder: GPUCommandEncoder,
  view: GPUTextureView,
  format: GPUTextureFormat,
  pixelWidth: number,
  pixelHeight: number,
): void {
  if (overlay === undefined) {
    return;
  }

  overlay.encode(encoder, view, format, pixelWidth, pixelHeight);
}
