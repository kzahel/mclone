const RGBA_COMPONENTS = 4;
const OFFSET_A = 24;
const OFFSET_B = 16;
const OFFSET_G = 8;
const OFFSET_R = 0;

function assertInsideBounds(width: number, height: number, x: number, y: number): void {
  if (x < 0 || x >= width || y < 0 || y >= height) {
    throw new Error(`(${x}, ${y}) outside of image bounds (${width}, ${height})`);
  }
}

export class NativeImage {
  private pixels: Uint8Array;
  private dataView: DataView;
  private closed = false;

  public constructor(
    private readonly width: number,
    private readonly height: number,
    clear: boolean,
  ) {
    if (width <= 0 || height <= 0) {
      throw new Error(`Invalid texture size: ${width}x${height}`);
    }

    this.pixels = new Uint8Array(width * height * RGBA_COMPONENTS);
    this.dataView = new DataView(this.pixels.buffer);
    if (clear) {
      this.pixels.fill(0);
    }
  }

  public static fromImageData(imageData: ImageData): NativeImage {
    const image = new NativeImage(imageData.width, imageData.height, false);
    image.pixels.set(imageData.data);
    return image;
  }

  public static fromRgbaPixels(width: number, height: number, pixels: Uint8Array): NativeImage {
    if (pixels.byteLength !== width * height * RGBA_COMPONENTS) {
      throw new Error(`Invalid RGBA pixel buffer size for ${width}x${height}: ${pixels.byteLength.toString()} bytes`);
    }

    const image = new NativeImage(width, height, false);
    image.pixels.set(pixels);
    return image;
  }

  public static getA(value: number): number {
    return (value >>> OFFSET_A) & 0xff;
  }

  public static getR(value: number): number {
    return (value >>> OFFSET_R) & 0xff;
  }

  public static getG(value: number): number {
    return (value >>> OFFSET_G) & 0xff;
  }

  public static getB(value: number): number {
    return (value >>> OFFSET_B) & 0xff;
  }

  public static combine(alpha: number, blue: number, green: number, red: number): number {
    return (((alpha & 0xff) << OFFSET_A) | ((blue & 0xff) << OFFSET_B) | ((green & 0xff) << OFFSET_G) | ((red & 0xff) << OFFSET_R)) >>> 0;
  }

  private checkAllocated(): void {
    if (this.closed) {
      throw new Error("Image is not allocated.");
    }
  }

  private pixelOffset(x: number, y: number): number {
    return (x + (y * this.width)) * RGBA_COMPONENTS;
  }

  private subImageBytes(x: number, y: number, width: number, height: number): Uint8Array {
    const bytes = new Uint8Array(width * height * RGBA_COMPONENTS);
    for (let row = 0; row < height; row++) {
      const sourceStart = this.pixelOffset(x, y + row);
      const targetStart = row * width * RGBA_COMPONENTS;
      bytes.set(this.pixels.subarray(sourceStart, sourceStart + (width * RGBA_COMPONENTS)), targetStart);
    }

    return bytes;
  }

  public close(): void {
    this.closed = true;
    this.pixels = new Uint8Array(0);
    this.dataView = new DataView(this.pixels.buffer);
  }

  public getWidth(): number {
    return this.width;
  }

  public getHeight(): number {
    return this.height;
  }

  public getPixelRGBA(x: number, y: number): number {
    assertInsideBounds(this.width, this.height, x, y);
    this.checkAllocated();
    return this.dataView.getUint32(this.pixelOffset(x, y), true);
  }

  public setPixelRGBA(x: number, y: number, value: number): void {
    assertInsideBounds(this.width, this.height, x, y);
    this.checkAllocated();
    this.dataView.setUint32(this.pixelOffset(x, y), value >>> 0, true);
  }

  public blendPixel(x: number, y: number, value: number): void {
    const existing = this.getPixelRGBA(x, y);
    const valueAlpha = NativeImage.getA(value) / 255.0;
    const valueBlue = NativeImage.getB(value) / 255.0;
    const valueGreen = NativeImage.getG(value) / 255.0;
    const valueRed = NativeImage.getR(value) / 255.0;
    const existingAlpha = NativeImage.getA(existing) / 255.0;
    const existingBlue = NativeImage.getB(existing) / 255.0;
    const existingGreen = NativeImage.getG(existing) / 255.0;
    const existingRed = NativeImage.getR(existing) / 255.0;
    const inverseAlpha = 1.0 - valueAlpha;

    let alpha = valueAlpha * valueAlpha + existingAlpha * inverseAlpha;
    let blue = valueBlue * valueAlpha + existingBlue * inverseAlpha;
    let green = valueGreen * valueAlpha + existingGreen * inverseAlpha;
    let red = valueRed * valueAlpha + existingRed * inverseAlpha;

    alpha = Math.min(1.0, alpha);
    blue = Math.min(1.0, blue);
    green = Math.min(1.0, green);
    red = Math.min(1.0, red);

    this.setPixelRGBA(
      x,
      y,
      NativeImage.combine(
        Math.trunc(alpha * 255.0),
        Math.trunc(blue * 255.0),
        Math.trunc(green * 255.0),
        Math.trunc(red * 255.0),
      ),
    );
  }

  public makePixelArray(): number[] {
    const pixels = new Array<number>(this.width * this.height);
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const pixel = this.getPixelRGBA(x, y);
        const alpha = NativeImage.getA(pixel);
        const blue = NativeImage.getB(pixel);
        const green = NativeImage.getG(pixel);
        const red = NativeImage.getR(pixel);
        pixels[x + (y * this.width)] = ((alpha << 24) | (red << 16) | (green << 8) | blue) >>> 0;
      }
    }

    return pixels;
  }

  public copyFrom(other: NativeImage): void {
    this.checkAllocated();
    other.checkAllocated();
    const width = Math.min(this.width, other.width);
    const height = Math.min(this.height, other.height);
    if (this.width === other.width) {
      this.pixels.set(other.pixels.subarray(0, Math.min(this.pixels.length, other.pixels.length)));
      return;
    }

    for (let row = 0; row < height; row++) {
      const sourceStart = row * other.width * RGBA_COMPONENTS;
      const targetStart = row * this.width * RGBA_COMPONENTS;
      this.pixels.set(other.pixels.subarray(sourceStart, sourceStart + (width * RGBA_COMPONENTS)), targetStart);
    }
  }

  public fillRect(x: number, y: number, width: number, height: number, value: number): void {
    for (let row = y; row < y + height; row++) {
      for (let column = x; column < x + width; column++) {
        this.setPixelRGBA(column, row, value);
      }
    }
  }

  public copyRect(x: number, y: number, translateX: number, translateY: number, width: number, height: number, mirrorX: boolean, mirrorY: boolean): void {
    for (let row = 0; row < height; row++) {
      for (let column = 0; column < width; column++) {
        const targetX = mirrorX ? width - 1 - column : column;
        const targetY = mirrorY ? height - 1 - row : row;
        const pixel = this.getPixelRGBA(x + column, y + row);
        this.setPixelRGBA(x + translateX + targetX, y + translateY + targetY, pixel);
      }
    }
  }

  // WebGPU: destination queue/texture are explicit instead of uploading into the currently bound GL texture.
  public upload(
    queue: GPUQueue,
    texture: GPUTexture,
    mipLevel: number,
    xOffset: number,
    yOffset: number,
    x: number,
    y: number,
    width: number,
    height: number,
  ): void {
    this.checkAllocated();
    const bytes = this.subImageBytes(x, y, width, height);
    const uploadBytes = new Uint8Array(bytes.length);
    uploadBytes.set(bytes);
    queue.writeTexture(
      {
        texture,
        mipLevel,
        origin: { x: xOffset, y: yOffset },
      },
      uploadBytes,
      {
        bytesPerRow: width * RGBA_COMPONENTS,
        rowsPerImage: height,
      },
      { width, height },
    );
  }
}
