import type { Font } from "../font";
import { AbstractButton } from "./abstract-button";

export type CycleButtonFormatter<T> = (value: T) => string;
export type CycleButtonOnValueChange<T> = (button: CycleButton<T>, value: T) => void;

export class CycleButton<T> extends AbstractButton {
  private valueIndex: number;

  public constructor(
    x: number,
    y: number,
    width: number,
    height: number,
    private readonly name: string,
    font: Font,
    private readonly values: readonly T[],
    value: T,
    private readonly formatter: CycleButtonFormatter<T>,
    private readonly onValueChange: CycleButtonOnValueChange<T>,
  ) {
    super(x, y, width, height, "", font);
    if (values.length === 0) {
      throw new Error("CycleButton requires at least one value");
    }
    const index = values.indexOf(value);
    this.valueIndex = index >= 0 ? index : 0;
    this.updateMessage();
  }

  public override onPress(): void {
    this.cycleValue(1);
  }

  public getValue(): T {
    return this.values[this.valueIndex]!;
  }

  public setValue(value: T): void {
    const index = this.values.indexOf(value);
    if (index < 0 || index === this.valueIndex) {
      return;
    }

    this.valueIndex = index;
    this.updateMessage();
    this.onValueChange(this, this.getValue());
  }

  public mouseScrolled(_mouseX: number, _mouseY: number, delta: number): boolean {
    if (!this.active || !this.visible) {
      return false;
    }
    this.cycleValue(delta < 0 ? 1 : -1);
    return true;
  }

  private cycleValue(amount: number): void {
    this.valueIndex = positiveModulo(this.valueIndex + amount, this.values.length);
    this.updateMessage();
    this.onValueChange(this, this.getValue());
  }

  private updateMessage(): void {
    this.setMessage(`${this.name}: ${this.formatter(this.getValue())}`);
  }
}

function positiveModulo(value: number, modulo: number): number {
  return ((value % modulo) + modulo) % modulo;
}
