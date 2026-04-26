import {
  createOffscreenTextureTarget,
  readTextureRgba8,
  requestWebGpuDeviceContext,
} from "../src/renderer/webgpu-target.ts";
import { encodePngRgba } from "./png-rgba.ts";

const WIDTH = 64;
const HEIGHT = 64;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-webgpu-smoke.png";
const CLEAR_COLOR = { r: 0.1, g: 0.4, b: 0.8, a: 1 } satisfies GPUColorDict;
const EXPECTED_PIXEL = new Uint8Array([26, 102, 204, 255]);
const PIXEL_TOLERANCE = 1;

const contextResult = await requestWebGpuDeviceContext();
if (!contextResult.ok) {
  throw new Error(contextResult.reason);
}

const { adapter, device } = contextResult.context;
const target = createOffscreenTextureTarget(device, WIDTH, HEIGHT, FORMAT);

const encoder = device.createCommandEncoder();
const pass = encoder.beginRenderPass({
  colorAttachments: [{
    view: target.view,
    clearValue: CLEAR_COLOR,
    loadOp: "clear",
    storeOp: "store",
  }],
});
pass.end();
device.queue.submit([encoder.finish()]);

const pixels = await readTextureRgba8(device, target.texture, target.width, target.height);
target.texture.destroy();

const firstPixel = pixels.slice(0, 4);
assertPixel(firstPixel, EXPECTED_PIXEL, PIXEL_TOLERANCE);
await Deno.writeFile(OUTPUT_PATH, encodePngRgba(target.width, target.height, pixels));

console.log(JSON.stringify({
  ok: true,
  outputPath: OUTPUT_PATH,
  width: target.width,
  height: target.height,
  format: target.format,
  firstPixel: Array.from(firstPixel),
  byteLength: pixels.byteLength,
  adapter: adapter.info ?? {},
}));

function assertPixel(actual: Uint8Array, expected: Uint8Array, tolerance: number): void {
  for (let i = 0; i < expected.length; i++) {
    const actualValue = actual[i];
    const expectedValue = expected[i]!;
    if (actualValue === undefined || Math.abs(actualValue - expectedValue) > tolerance) {
      throw new Error(
        `Deno WebGPU smoke pixel mismatch at channel ${i.toString()}: expected ${expectedValue.toString()} +/- ${tolerance.toString()}, got ${actualValue?.toString() ?? "undefined"}`,
      );
    }
  }
}
