export abstract class VertexConsumer {
  public vertex(x: number, y: number, z: number): this;
  public vertex(
    x: number,
    y: number,
    z: number,
    r: number,
    g: number,
    b: number,
    a: number,
    u: number,
    v: number,
    overlay: number,
    light: number,
    normalX: number,
    normalY: number,
    normalZ: number,
  ): void;
  public vertex(...args: number[]): this | void {
    if (args.length === 3) {
      return this.vertexPosition(args[0]!, args[1]!, args[2]!);
    }

    this.vertexPosition(args[0]!, args[1]!, args[2]!);
    this.colorFloat(args[3]!, args[4]!, args[5]!, args[6]!);
    this.uv(args[7]!, args[8]!);
    this.overlayCoords(args[9]!);
    this.uv2(args[10]!);
    this.normal(args[11]!, args[12]!, args[13]!);
    this.endVertex();
  }

  protected abstract vertexPosition(x: number, y: number, z: number): this;

  public abstract color(r: number, g: number, b: number, a: number): this;

  public abstract uv(u: number, v: number): this;

  public overlayCoords(overlay: number): this;
  public overlayCoords(u: number, v: number): this;
  public overlayCoords(uOrOverlay: number, v?: number): this {
    if (v === undefined) {
      return this.overlayCoordsPair(uOrOverlay & 0xffff, (uOrOverlay >> 16) & 0xffff);
    }

    return this.overlayCoordsPair(uOrOverlay, v);
  }

  protected abstract overlayCoordsPair(u: number, v: number): this;

  public uv2(light: number): this;
  public uv2(u: number, v: number): this;
  public uv2(uOrLight: number, v?: number): this {
    if (v === undefined) {
      return this.uv2Pair(uOrLight & 0xffff, (uOrLight >> 16) & 0xffff);
    }

    return this.uv2Pair(uOrLight, v);
  }

  protected abstract uv2Pair(u: number, v: number): this;

  public abstract normal(x: number, y: number, z: number): this;

  public abstract endVertex(): void;

  public abstract defaultColor(r: number, g: number, b: number, a: number): void;

  public abstract unsetDefaultColor(): void;

  public colorFloat(r: number, g: number, b: number, a: number): this {
    return this.color(Math.trunc(r * 255), Math.trunc(g * 255), Math.trunc(b * 255), Math.trunc(a * 255));
  }
}
