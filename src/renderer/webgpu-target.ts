export interface WebGpuDeviceContext {
  readonly adapter: GPUAdapter;
  readonly device: GPUDevice;
  readonly format: GPUTextureFormat;
}

export type WebGpuDeviceContextResult =
  | { readonly ok: true; readonly context: WebGpuDeviceContext }
  | { readonly ok: false; readonly reason: string };

export interface BrowserCanvasTarget {
  readonly canvas: HTMLCanvasElement;
  readonly ctx: GPUCanvasContext;
  readonly format: GPUTextureFormat;
}

export interface OffscreenTextureTarget {
  readonly texture: GPUTexture;
  readonly view: GPUTextureView;
  readonly width: number;
  readonly height: number;
  readonly format: GPUTextureFormat;
}

export interface WebGpuDeviceContextProgress {
  readonly onRequestAdapter?: () => void;
  readonly onRequestDevice?: () => void;
}

const WEBGPU_COPY_BYTES_PER_ROW_ALIGNMENT = 256;

export async function requestWebGpuDeviceContext(
  progress: WebGpuDeviceContextProgress = {},
): Promise<WebGpuDeviceContextResult> {
  if (!navigator.gpu) {
    return { ok: false, reason: "navigator.gpu missing (no WebGPU)" };
  }

  progress.onRequestAdapter?.();
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) {
    return { ok: false, reason: "requestAdapter returned null" };
  }

  progress.onRequestDevice?.();
  const device = await adapter.requestDevice();
  return {
    ok: true,
    context: {
      adapter,
      device,
      format: navigator.gpu.getPreferredCanvasFormat(),
    },
  };
}

export function configureBrowserCanvasTarget(
  canvas: HTMLCanvasElement,
  device: GPUDevice,
  format: GPUTextureFormat,
): BrowserCanvasTarget | undefined {
  const ctx = canvas.getContext("webgpu");
  if (!ctx) {
    return undefined;
  }

  ctx.configure({ device, format, alphaMode: "opaque" });
  return { canvas, ctx, format };
}

export function createOffscreenTextureTarget(
  device: GPUDevice,
  width: number,
  height: number,
  format: GPUTextureFormat,
  usage: GPUTextureUsageFlags = GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
): OffscreenTextureTarget {
  const texture = device.createTexture({
    size: { width, height, depthOrArrayLayers: 1 },
    format,
    usage,
  });
  return {
    texture,
    view: texture.createView(),
    width,
    height,
    format,
  };
}

export async function readTextureRgba8(
  device: GPUDevice,
  texture: GPUTexture,
  width: number,
  height: number,
): Promise<Uint8Array> {
  const bytesPerPixel = 4;
  const unpaddedBytesPerRow = width * bytesPerPixel;
  const bytesPerRow = alignTo(unpaddedBytesPerRow, WEBGPU_COPY_BYTES_PER_ROW_ALIGNMENT);
  const readback = device.createBuffer({
    size: bytesPerRow * height,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  const encoder = device.createCommandEncoder();
  encoder.copyTextureToBuffer(
    { texture },
    { buffer: readback, bytesPerRow, rowsPerImage: height },
    { width, height, depthOrArrayLayers: 1 },
  );
  device.queue.submit([encoder.finish()]);
  await device.queue.onSubmittedWorkDone();
  await readback.mapAsync(GPUMapMode.READ);

  const padded = new Uint8Array(readback.getMappedRange());
  const pixels = new Uint8Array(unpaddedBytesPerRow * height);
  for (let row = 0; row < height; row++) {
    const paddedOffset = row * bytesPerRow;
    const pixelOffset = row * unpaddedBytesPerRow;
    pixels.set(padded.subarray(paddedOffset, paddedOffset + unpaddedBytesPerRow), pixelOffset);
  }

  readback.unmap();
  readback.destroy();
  return pixels;
}

function alignTo(value: number, alignment: number): number {
  return Math.ceil(value / alignment) * alignment;
}
