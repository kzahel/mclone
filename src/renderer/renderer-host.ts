import { createRenderWorldWorker, type RenderWorldWorkerClientEndpoint } from "./chunk/render-world-worker-client";
import {
  configureBrowserCanvasTarget,
  createOffscreenTextureTarget,
  requestWebGpuDeviceContext,
  type BrowserCanvasTarget,
  type OffscreenTextureTarget,
  type WebGpuDeviceContextProgress,
  type WebGpuDeviceContextResult,
} from "./webgpu-target";

export interface RenderWorldWorkerEndpointFactory {
  createRenderWorldWorkerEndpoint(): RenderWorldWorkerClientEndpoint;
}

export interface WebGpuDeviceContextProvider {
  requestWebGpuDeviceContext(progress?: WebGpuDeviceContextProgress): Promise<WebGpuDeviceContextResult>;
}

export interface BrowserRendererHost extends WebGpuDeviceContextProvider, RenderWorldWorkerEndpointFactory {
  createCanvasTarget(canvas: HTMLCanvasElement, device: GPUDevice, format: GPUTextureFormat): BrowserCanvasTarget | undefined;
}

export interface HeadlessRendererHost extends WebGpuDeviceContextProvider, RenderWorldWorkerEndpointFactory {
  createOffscreenTarget(
    device: GPUDevice,
    width: number,
    height: number,
    format: GPUTextureFormat,
    usage?: GPUTextureUsageFlags,
  ): OffscreenTextureTarget;
}

export function createBrowserRendererHost(
  createRenderWorldWorkerEndpoint: () => RenderWorldWorkerClientEndpoint = createRenderWorldWorker,
): BrowserRendererHost {
  return {
    requestWebGpuDeviceContext,
    createCanvasTarget: configureBrowserCanvasTarget,
    createRenderWorldWorkerEndpoint,
  };
}

export function createHeadlessRendererHost(
  createRenderWorldWorkerEndpoint: () => RenderWorldWorkerClientEndpoint,
): HeadlessRendererHost {
  return {
    requestWebGpuDeviceContext,
    createOffscreenTarget: createOffscreenTextureTarget,
    createRenderWorldWorkerEndpoint,
  };
}

export const BROWSER_RENDERER_HOST: BrowserRendererHost = createBrowserRendererHost();
