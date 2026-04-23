export abstract class FeatureSize {
  public constructor(protected readonly minClippedHeightValue?: number) {}

  public abstract getSizeAtHeight(treeHeight: number, height: number): number;

  public minClippedHeight(): number | undefined {
    return this.minClippedHeightValue;
  }
}
