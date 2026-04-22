import { VertexConsumer } from "./vertex-consumer";

export abstract class DefaultedVertexConsumer extends VertexConsumer {
  protected defaultColorSet = false;
  protected defaultR = 255;
  protected defaultG = 255;
  protected defaultB = 255;
  protected defaultA = 255;

  public defaultColor(r: number, g: number, b: number, a: number): void {
    this.defaultR = r;
    this.defaultG = g;
    this.defaultB = b;
    this.defaultA = a;
    this.defaultColorSet = true;
  }

  public unsetDefaultColor(): void {
    this.defaultColorSet = false;
  }
}
