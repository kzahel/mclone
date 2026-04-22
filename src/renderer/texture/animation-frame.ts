export class AnimationFrame {
  public static readonly UNKNOWN_FRAME_TIME = -1;

  public constructor(
    private readonly index: number,
    private readonly time: number = AnimationFrame.UNKNOWN_FRAME_TIME,
  ) {}

  public getTime(defaultFrameTime: number): number {
    return this.time === AnimationFrame.UNKNOWN_FRAME_TIME ? defaultFrameTime : this.time;
  }

  public getIndex(): number {
    return this.index;
  }
}
