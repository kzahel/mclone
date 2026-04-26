import { NativeImage } from "./native-image";
import type { NativeImageDecoder } from "./native-image-decoder";

const PNG_SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a] as const;
const RGBA_COMPONENTS = 4;

interface PngHeader {
  readonly width: number;
  readonly height: number;
  readonly bitDepth: number;
  readonly colorType: number;
  readonly compressionMethod: number;
  readonly filterMethod: number;
  readonly interlaceMethod: number;
}

export class PngNativeImageDecoder implements NativeImageDecoder {
  public async decode(blob: Blob): Promise<NativeImage> {
    return decodePngNativeImage(new Uint8Array(await blob.arrayBuffer()));
  }
}

export async function decodePngNativeImage(bytes: Uint8Array): Promise<NativeImage> {
  const { header, idat } = parsePng(bytes);
  validateSupportedHeader(header);
  const inflated = await inflateZlib(idat);
  return NativeImage.fromRgbaPixels(header.width, header.height, unfilterRgbaRows(header.width, header.height, inflated));
}

function parsePng(bytes: Uint8Array): { readonly header: PngHeader; readonly idat: Uint8Array } {
  if (bytes.byteLength < PNG_SIGNATURE.length) {
    throw new Error("PNG data is shorter than the signature");
  }

  for (let i = 0; i < PNG_SIGNATURE.length; i++) {
    if (bytes[i] !== PNG_SIGNATURE[i]) {
      throw new Error("Invalid PNG signature");
    }
  }

  let offset = PNG_SIGNATURE.length;
  let header: PngHeader | undefined;
  const idatChunks: Uint8Array[] = [];
  while (offset < bytes.byteLength) {
    if (offset + 8 > bytes.byteLength) {
      throw new Error("Truncated PNG chunk header");
    }

    const length = readU32(bytes, offset);
    const type = readAscii(bytes, offset + 4, 4);
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    if (dataEnd + 4 > bytes.byteLength) {
      throw new Error(`Truncated PNG chunk ${type}`);
    }

    const data = bytes.subarray(dataStart, dataEnd);
    if (type === "IHDR") {
      header = parseHeader(data);
    } else if (type === "IDAT") {
      idatChunks.push(data);
    } else if (type === "IEND") {
      break;
    }

    offset = dataEnd + 4;
  }

  if (header === undefined) {
    throw new Error("PNG is missing IHDR");
  }
  if (idatChunks.length === 0) {
    throw new Error("PNG is missing IDAT");
  }

  return { header, idat: concatBytes(idatChunks) };
}

function parseHeader(data: Uint8Array): PngHeader {
  if (data.byteLength !== 13) {
    throw new Error(`Invalid IHDR length ${data.byteLength.toString()}`);
  }

  return {
    width: readU32(data, 0),
    height: readU32(data, 4),
    bitDepth: data[8]!,
    colorType: data[9]!,
    compressionMethod: data[10]!,
    filterMethod: data[11]!,
    interlaceMethod: data[12]!,
  };
}

function validateSupportedHeader(header: PngHeader): void {
  if (header.width <= 0 || header.height <= 0) {
    throw new Error(`Invalid PNG size ${header.width.toString()}x${header.height.toString()}`);
  }
  if (header.bitDepth !== 8 || header.colorType !== 6) {
    throw new Error(`Unsupported PNG format: bitDepth=${header.bitDepth.toString()} colorType=${header.colorType.toString()}`);
  }
  if (header.compressionMethod !== 0 || header.filterMethod !== 0 || header.interlaceMethod !== 0) {
    throw new Error(
      `Unsupported PNG methods: compression=${header.compressionMethod.toString()} filter=${header.filterMethod.toString()} interlace=${header.interlaceMethod.toString()}`,
    );
  }
}

async function inflateZlib(bytes: Uint8Array): Promise<Uint8Array> {
  if (typeof DecompressionStream === "function") {
    const input = new Uint8Array(bytes.byteLength);
    input.set(bytes);
    const stream = new Blob([input.buffer as ArrayBuffer]).stream().pipeThrough(new DecompressionStream("deflate"));
    return new Uint8Array(await new Response(stream).arrayBuffer());
  }

  return inflateStoredZlib(bytes);
}

function inflateStoredZlib(bytes: Uint8Array): Uint8Array {
  if (bytes.byteLength < 6) {
    throw new Error("Stored zlib stream is too short");
  }
  const compressionMethod = bytes[0]! & 0x0f;
  const compressionInfo = bytes[0]! >>> 4;
  if (compressionMethod !== 8 || compressionInfo > 7) {
    throw new Error("Unsupported zlib header");
  }
  if ((((bytes[0]! << 8) + bytes[1]!) % 31) !== 0) {
    throw new Error("Invalid zlib header checksum");
  }
  if ((bytes[1]! & 0x20) !== 0) {
    throw new Error("Preset zlib dictionaries are not supported");
  }

  const chunks: Uint8Array[] = [];
  let bitOffset = 16;
  for (;;) {
    const finalBlock = readBits(bytes, bitOffset, 1) === 1;
    bitOffset += 1;
    const blockType = readBits(bytes, bitOffset, 2);
    bitOffset += 2;
    if (blockType !== 0) {
      throw new Error("CompressionStream or DecompressionStream is required for compressed PNG IDAT data");
    }

    bitOffset = Math.ceil(bitOffset / 8) * 8;
    const byteOffset = bitOffset / 8;
    if (byteOffset + 4 > bytes.byteLength) {
      throw new Error("Truncated stored zlib block header");
    }
    const length = bytes[byteOffset]! | (bytes[byteOffset + 1]! << 8);
    const nlength = bytes[byteOffset + 2]! | (bytes[byteOffset + 3]! << 8);
    if (((length ^ 0xffff) & 0xffff) !== nlength) {
      throw new Error("Invalid stored zlib block length");
    }
    const dataStart = byteOffset + 4;
    const dataEnd = dataStart + length;
    if (dataEnd > bytes.byteLength - 4) {
      throw new Error("Truncated stored zlib block data");
    }
    chunks.push(bytes.subarray(dataStart, dataEnd));
    bitOffset = dataEnd * 8;
    if (finalBlock) {
      break;
    }
  }

  return concatBytes(chunks);
}

function unfilterRgbaRows(width: number, height: number, bytes: Uint8Array): Uint8Array {
  const rowStride = width * RGBA_COMPONENTS;
  const expectedSize = (rowStride + 1) * height;
  if (bytes.byteLength !== expectedSize) {
    throw new Error(`Inflated PNG payload size mismatch: expected ${expectedSize.toString()} bytes, got ${bytes.byteLength.toString()}`);
  }

  const pixels = new Uint8Array(rowStride * height);
  let inputOffset = 0;
  for (let y = 0; y < height; y++) {
    const filter = bytes[inputOffset++]!;
    const rowOffset = y * rowStride;
    const previousRowOffset = rowOffset - rowStride;
    for (let x = 0; x < rowStride; x++) {
      const raw = bytes[inputOffset++]!;
      const left = x >= RGBA_COMPONENTS ? pixels[rowOffset + x - RGBA_COMPONENTS]! : 0;
      const up = y > 0 ? pixels[previousRowOffset + x]! : 0;
      const upLeft = y > 0 && x >= RGBA_COMPONENTS ? pixels[previousRowOffset + x - RGBA_COMPONENTS]! : 0;
      pixels[rowOffset + x] = unfilterByte(filter, raw, left, up, upLeft);
    }
  }

  return pixels;
}

function unfilterByte(filter: number, raw: number, left: number, up: number, upLeft: number): number {
  switch (filter) {
    case 0:
      return raw;
    case 1:
      return (raw + left) & 0xff;
    case 2:
      return (raw + up) & 0xff;
    case 3:
      return (raw + Math.floor((left + up) / 2)) & 0xff;
    case 4:
      return (raw + paethPredictor(left, up, upLeft)) & 0xff;
    default:
      throw new Error(`Unsupported PNG filter type ${filter.toString()}`);
  }
}

function paethPredictor(left: number, up: number, upLeft: number): number {
  const p = left + up - upLeft;
  const pa = Math.abs(p - left);
  const pb = Math.abs(p - up);
  const pc = Math.abs(p - upLeft);
  if (pa <= pb && pa <= pc) return left;
  if (pb <= pc) return up;
  return upLeft;
}

function readBits(bytes: Uint8Array, bitOffset: number, bitCount: number): number {
  let value = 0;
  for (let bit = 0; bit < bitCount; bit++) {
    const sourceBit = bitOffset + bit;
    const byte = bytes[sourceBit >>> 3]!;
    value |= ((byte >>> (sourceBit & 7)) & 1) << bit;
  }
  return value;
}

function readU32(bytes: Uint8Array, offset: number): number {
  return (
    ((bytes[offset]! << 24) >>> 0)
    | (bytes[offset + 1]! << 16)
    | (bytes[offset + 2]! << 8)
    | bytes[offset + 3]!
  ) >>> 0;
}

function readAscii(bytes: Uint8Array, offset: number, length: number): string {
  let value = "";
  for (let i = 0; i < length; i++) {
    value += String.fromCharCode(bytes[offset + i]!);
  }
  return value;
}

function concatBytes(parts: readonly Uint8Array[]): Uint8Array {
  const total = parts.reduce((sum, part) => sum + part.byteLength, 0);
  const result = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.byteLength;
  }
  return result;
}
