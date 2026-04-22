import { AnimationFrame } from "./animation-frame";

function isDivisionInteger(dividend: number, divisor: number): boolean {
  return Math.trunc(dividend / divisor) * divisor === dividend;
}

export class AnimationMetadataSection {
  public static readonly DEFAULT_FRAME_TIME = 1;
  public static readonly UNKNOWN_SIZE = -1;
  public static readonly EMPTY = new AnimationMetadataSection([], -1, -1, 1, false, true);

  public constructor(
    private readonly frames: readonly AnimationFrame[],
    private readonly frameWidth: number,
    private readonly frameHeight: number,
    private readonly defaultFrameTime: number,
    private readonly interpolatedFrames: boolean,
    private readonly empty: boolean = false,
  ) {}

  private calculateFrameSize(width: number, height: number): readonly [number, number] {
    if (this.frameWidth !== AnimationMetadataSection.UNKNOWN_SIZE) {
      return this.frameHeight !== AnimationMetadataSection.UNKNOWN_SIZE ? [this.frameWidth, this.frameHeight] : [this.frameWidth, height];
    }

    if (this.frameHeight !== AnimationMetadataSection.UNKNOWN_SIZE) {
      return [width, this.frameHeight];
    }

    const size = Math.min(width, height);
    return [size, size];
  }

  public getFrameSize(width: number, height: number): readonly [number, number] {
    if (this.empty) {
      return [width, height];
    }

    const [frameWidth, frameHeight] = this.calculateFrameSize(width, height);
    if (isDivisionInteger(width, frameWidth) && isDivisionInteger(height, frameHeight)) {
      return [frameWidth, frameHeight];
    }

    throw new Error(`Image size ${width},${height} is not multiply of frame size ${frameWidth},${frameHeight}`);
  }

  public getFrameHeight(height: number): number {
    return this.frameHeight === AnimationMetadataSection.UNKNOWN_SIZE ? height : this.frameHeight;
  }

  public getFrameWidth(width: number): number {
    return this.frameWidth === AnimationMetadataSection.UNKNOWN_SIZE ? width : this.frameWidth;
  }

  public getDefaultFrameTime(): number {
    return this.defaultFrameTime;
  }

  public isInterpolatedFrames(): boolean {
    return this.interpolatedFrames;
  }

  public forEachFrame(output: (index: number, time: number) => void): void {
    for (const frame of this.frames) {
      output(frame.getIndex(), frame.getTime(this.defaultFrameTime));
    }
  }
}
