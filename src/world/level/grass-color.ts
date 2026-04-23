export class GrassColor {
  private static pixels = new Int32Array(65_536);

  public static init(pixels: readonly number[]): void {
    this.pixels = Int32Array.from(pixels);
  }

  public static get(temperature: number, downfall: number): number {
    const adjustedDownfall = downfall * temperature;
    const x = Math.trunc((1.0 - temperature) * 255.0);
    const y = Math.trunc((1.0 - adjustedDownfall) * 255.0);
    const index = (y << 8) | x;
    return index >= this.pixels.length ? -65_281 : this.pixels[index]!;
  }
}
