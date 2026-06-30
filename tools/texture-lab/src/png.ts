import zlib from "node:zlib";

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

export interface RgbaImage {
  width: number;
  height: number;
  data: Uint8Array;
}

export function decodePng(buffer: Uint8Array): RgbaImage {
  const bytes = Buffer.from(buffer);
  if (bytes.length < PNG_SIGNATURE.length || !bytes.subarray(0, PNG_SIGNATURE.length).equals(PNG_SIGNATURE)) {
    throw new Error("Invalid PNG signature");
  }

  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  let interlace = 0;
  const idatChunks: Buffer[] = [];
  let palette: Buffer | undefined;
  let paletteAlpha: Buffer | undefined;

  let offset = PNG_SIGNATURE.length;
  while (offset < bytes.length) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.subarray(offset + 4, offset + 8).toString("ascii");
    const data = bytes.subarray(offset + 8, offset + 8 + length);
    offset += 12 + length;

    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8]!;
      colorType = data[9]!;
      interlace = data[12]!;
    } else if (type === "IDAT") {
      idatChunks.push(data);
    } else if (type === "PLTE") {
      palette = Buffer.from(data);
    } else if (type === "tRNS") {
      paletteAlpha = Buffer.from(data);
    } else if (type === "IEND") {
      break;
    }
  }

  if (width <= 0 || height <= 0) {
    throw new Error("PNG is missing IHDR dimensions");
  }
  if (
    bitDepth !== 8 ||
    interlace !== 0 ||
    (colorType !== 6 && colorType !== 4 && colorType !== 3 && colorType !== 2 && colorType !== 0)
  ) {
    throw new Error(`Unsupported PNG format: bitDepth=${bitDepth} colorType=${colorType} interlace=${interlace}`);
  }
  if (colorType === 3 && (!palette || palette.length === 0 || palette.length % 3 !== 0)) {
    throw new Error("Indexed PNG is missing a valid PLTE chunk");
  }

  const bytesPerPixel = colorType === 6 ? 4 : colorType === 4 ? 2 : colorType === 2 ? 3 : 1;
  const scanlineLength = width * bytesPerPixel;
  const inflated = zlib.inflateSync(Buffer.concat(idatChunks));
  const expectedLength = height * (scanlineLength + 1);
  if (inflated.length < expectedLength) {
    throw new Error(`PNG IDAT data is too short: ${inflated.length}, expected ${expectedLength}`);
  }

  const rows = Buffer.alloc(height * scanlineLength);
  const previous = Buffer.alloc(scanlineLength);
  const current = Buffer.alloc(scanlineLength);

  for (let y = 0; y < height; y += 1) {
    const sourceOffset = y * (scanlineLength + 1);
    const filter = inflated[sourceOffset]!;
    inflated.copy(current, 0, sourceOffset + 1, sourceOffset + 1 + scanlineLength);
    unfilterScanline(current, previous, bytesPerPixel, filter);
    current.copy(rows, y * scanlineLength);
    current.copy(previous);
  }

  const data = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const source = y * scanlineLength + x * bytesPerPixel;
      const target = (y * width + x) * 4;
      if (colorType === 3) {
        const paletteIndex = rows[source]!;
        const paletteOffset = paletteIndex * 3;
        data[target] = palette![paletteOffset] ?? 0;
        data[target + 1] = palette![paletteOffset + 1] ?? 0;
        data[target + 2] = palette![paletteOffset + 2] ?? 0;
        data[target + 3] = paletteAlpha?.[paletteIndex] ?? 255;
      } else if (colorType === 4) {
        const gray = rows[source]!;
        data[target] = gray;
        data[target + 1] = gray;
        data[target + 2] = gray;
        data[target + 3] = rows[source + 1]!;
      } else if (colorType === 0) {
        const gray = rows[source]!;
        data[target] = gray;
        data[target + 1] = gray;
        data[target + 2] = gray;
        data[target + 3] = 255;
      } else {
        data[target] = rows[source]!;
        data[target + 1] = rows[source + 1]!;
        data[target + 2] = rows[source + 2]!;
        data[target + 3] = colorType === 6 ? rows[source + 3]! : 255;
      }
    }
  }

  return { width, height, data };
}

export function encodePng(image: RgbaImage): Buffer {
  if (image.data.length !== image.width * image.height * 4) {
    throw new Error(`RGBA buffer length ${image.data.length} does not match ${image.width}x${image.height}`);
  }

  const scanlineLength = image.width * 4;
  const filtered = Buffer.alloc((scanlineLength + 1) * image.height);
  for (let y = 0; y < image.height; y += 1) {
    const outputOffset = y * (scanlineLength + 1);
    filtered[outputOffset] = 0;
    filtered.set(image.data.subarray(y * scanlineLength, (y + 1) * scanlineLength), outputOffset + 1);
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(image.width, 0);
  ihdr.writeUInt32BE(image.height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  return Buffer.concat([
    PNG_SIGNATURE,
    chunk("IHDR", ihdr),
    chunk("IDAT", zlib.deflateSync(filtered, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function chunk(type: string, data: Buffer): Buffer {
  const typeBuffer = Buffer.from(type, "ascii");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function unfilterScanline(scanline: Buffer, previous: Buffer, bytesPerPixel: number, filter: number): void {
  for (let index = 0; index < scanline.length; index += 1) {
    const left = index >= bytesPerPixel ? scanline[index - bytesPerPixel]! : 0;
    const up = previous[index]!;
    const upLeft = index >= bytesPerPixel ? previous[index - bytesPerPixel]! : 0;
    if (filter === 0) {
      continue;
    }
    if (filter === 1) {
      scanline[index] = (scanline[index]! + left) & 0xff;
    } else if (filter === 2) {
      scanline[index] = (scanline[index]! + up) & 0xff;
    } else if (filter === 3) {
      scanline[index] = (scanline[index]! + Math.floor((left + up) / 2)) & 0xff;
    } else if (filter === 4) {
      scanline[index] = (scanline[index]! + paeth(left, up, upLeft)) & 0xff;
    } else {
      throw new Error(`Unsupported PNG filter ${filter}`);
    }
  }
}

function paeth(left: number, up: number, upLeft: number): number {
  const estimate = left + up - upLeft;
  const leftDistance = Math.abs(estimate - left);
  const upDistance = Math.abs(estimate - up);
  const upLeftDistance = Math.abs(estimate - upLeft);
  if (leftDistance <= upDistance && leftDistance <= upLeftDistance) {
    return left;
  }
  return upDistance <= upLeftDistance ? up : upLeft;
}

const CRC_TABLE = new Uint32Array(256).map((_, index) => {
  let value = index;
  for (let bit = 0; bit < 8; bit += 1) {
    value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
  }
  return value >>> 0;
});

function crc32(buffer: Buffer): number {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff]! ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
