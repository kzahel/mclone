import { FeatureSize } from "./feature-size";

export class ThreeLayersFeatureSize extends FeatureSize {
  public constructor(
    private readonly limit: number,
    private readonly upperLimit: number,
    private readonly lowerSize: number,
    private readonly middleSize: number,
    private readonly upperSize: number,
    minClippedHeight?: number,
  ) {
    super(minClippedHeight);
  }

  public override getSizeAtHeight(treeHeight: number, height: number): number {
    if (height < this.limit) {
      return this.lowerSize;
    }

    return height >= treeHeight - this.upperLimit ? this.upperSize : this.middleSize;
  }
}
