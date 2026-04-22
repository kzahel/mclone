import { DefaultedVertexConsumer } from "./defaulted-vertex-consumer";
import { VertexFormatElement } from "./vertex-format-element";

function clamp(value: number, minValue: number, maxValue: number): number {
  if (value < minValue) {
    return minValue;
  }

  return value > maxValue ? maxValue : value;
}

export abstract class BufferVertexConsumer extends DefaultedVertexConsumer {
  public abstract currentElement(): VertexFormatElement;

  public abstract nextElement(): void;

  public abstract putByte(index: number, value: number): void;

  public abstract putShort(index: number, value: number): void;

  public abstract putFloat(index: number, value: number): void;

  protected vertexPosition(x: number, y: number, z: number): this {
    const element = this.currentElement();
    if (element.getUsage() !== VertexFormatElement.Usage.POSITION) {
      return this;
    }

    if (element.getType() !== VertexFormatElement.Type.FLOAT || element.getCount() !== 3) {
      throw new Error("Illegal vertex position element");
    }

    this.putFloat(0, x);
    this.putFloat(4, y);
    this.putFloat(8, z);
    this.nextElement();
    return this;
  }

  public color(r: number, g: number, b: number, a: number): this {
    const element = this.currentElement();
    if (element.getUsage() !== VertexFormatElement.Usage.COLOR) {
      return this;
    }

    if (element.getType() !== VertexFormatElement.Type.UBYTE || element.getCount() !== 4) {
      throw new Error("Illegal vertex color element");
    }

    this.putByte(0, r);
    this.putByte(1, g);
    this.putByte(2, b);
    this.putByte(3, a);
    this.nextElement();
    return this;
  }

  public uv(u: number, v: number): this {
    const element = this.currentElement();
    if (element.getUsage() !== VertexFormatElement.Usage.UV || element.getIndex() !== 0) {
      return this;
    }

    if (element.getType() !== VertexFormatElement.Type.FLOAT || element.getCount() !== 2) {
      throw new Error("Illegal vertex uv element");
    }

    this.putFloat(0, u);
    this.putFloat(4, v);
    this.nextElement();
    return this;
  }

  protected overlayCoordsPair(u: number, v: number): this {
    return this.uvShort(u, v, 1);
  }

  protected uv2Pair(u: number, v: number): this {
    return this.uvShort(u, v, 2);
  }

  private uvShort(u: number, v: number, index: number): this {
    const element = this.currentElement();
    if (element.getUsage() !== VertexFormatElement.Usage.UV || element.getIndex() !== index) {
      return this;
    }

    if (element.getType() !== VertexFormatElement.Type.SHORT || element.getCount() !== 2) {
      throw new Error("Illegal vertex short uv element");
    }

    this.putShort(0, u);
    this.putShort(2, v);
    this.nextElement();
    return this;
  }

  public normal(x: number, y: number, z: number): this {
    const element = this.currentElement();
    if (element.getUsage() !== VertexFormatElement.Usage.NORMAL) {
      return this;
    }

    if (element.getType() !== VertexFormatElement.Type.BYTE || element.getCount() !== 3) {
      throw new Error("Illegal vertex normal element");
    }

    this.putByte(0, BufferVertexConsumer.normalIntValue(x));
    this.putByte(1, BufferVertexConsumer.normalIntValue(y));
    this.putByte(2, BufferVertexConsumer.normalIntValue(z));
    this.nextElement();
    return this;
  }

  public static normalIntValue(value: number): number {
    return (Math.trunc(clamp(value, -1, 1) * 127) & 0xff);
  }
}
