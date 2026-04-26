import {
  createOffscreenTextureTarget,
  readTextureRgba8,
  requestWebGpuDeviceContext,
} from "../src/renderer/webgpu-target.ts";

const WIDTH = 64;
const HEIGHT = 64;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-webgpu-smoke.png";
const CLEAR_COLOR = { r: 0.1, g: 0.4, b: 0.8, a: 1 } satisfies GPUColorDict;
const EXPECTED_PIXEL = new Uint8Array([26, 102, 204, 255]);

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
assertPixel(firstPixel, EXPECTED_PIXEL);
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

function assertPixel(actual: Uint8Array, expected: Uint8Array): void {
  for (let i = 0; i < expected.length; i++) {
    if (actual[i] !== expected[i]) {
      throw new Error(
        `Deno WebGPU smoke pixel mismatch at channel ${i.toString()}: expected ${expected[i]!.toString()}, got ${actual[i]?.toString() ?? "undefined"}`,
      );
    }
  }
}

function encodePngRgba(width: number, height: number, rgba: Uint8Array): Uint8Array {
  const raw = new Uint8Array((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    const rawOffset = y * (width * 4 + 1);
    const rgbaOffset = y * width * 4;
    raw[rawOffset] = 0;
    raw.set(rgba.subarray(rgbaOffset, rgbaOffset + width * 4), rawOffset + 1);
  }

  return concatBytes([
    new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    pngChunk("IHDR", concatBytes([
      u32be(width),
      u32be(height),
      new Uint8Array([8, 6, 0, 0, 0]),
    ])),
    pngChunk("IDAT", zlibStore(raw)),
    pngChunk("IEND", new Uint8Array()),
  ]);
}

function zlibStore(data: Uint8Array): Uint8Array {
  const chunks: Uint8Array[] = [new Uint8Array([0x78, 0x01])];
  let offset = 0;
  while (offset < data.length) {
    const blockLength = Math.min(data.length - offset, 0xffff);
    const finalBlock = offset + blockLength >= data.length;
    const header = new Uint8Array(5);
    header[0] = finalBlock ? 1 : 0;
    header[1] = blockLength & 0xff;
    header[2] = (blockLength >>> 8) & 0xff;
    const nlen = (~blockLength) & 0xffff;
    header[3] = nlen & 0xff;
    header[4] = (nlen >>> 8) & 0xff;
    chunks.push(header, data.subarray(offset, offset + blockLength));
    offset += blockLength;
  }
  chunks.push(u32be(adler32(data)));
  return concatBytes(chunks);
}

function pngChunk(type: string, data: Uint8Array): Uint8Array {
  const typeBytes = new TextEncoder().encode(type);
  const crcInput = concatBytes([typeBytes, data]);
  return concatBytes([
    u32be(data.length),
    typeBytes,
    data,
    u32be(crc32(crcInput)),
  ]);
}

function u32be(value: number): Uint8Array {
  return new Uint8Array([
    (value >>> 24) & 0xff,
    (value >>> 16) & 0xff,
    (value >>> 8) & 0xff,
    value & 0xff,
  ]);
}

function concatBytes(parts: readonly Uint8Array[]): Uint8Array {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

function adler32(data: Uint8Array): number {
  let a = 1;
  let b = 0;
  for (const byte of data) {
    a = (a + byte) % 65521;
    b = (b + a) % 65521;
  }
  return ((b << 16) | a) >>> 0;
}

function crc32(data: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of data) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      const mask = -(crc & 1);
      crc = (crc >>> 1) ^ (0xedb88320 & mask);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}
