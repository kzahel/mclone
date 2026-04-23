import { FeatureSize } from "./feature-size";

export class TwoLayersFeatureSize extends FeatureSize {
  public constructor(
    private readonly limit: number,
    private readonly lowerSize: number,
    private readonly upperSize: number,
    minClippedHeight?: number,
  ) {
    super(minClippedHeight);
  }

  public override getSizeAtHeight(_treeHeight: number, height: number): number {
    return height < this.limit ? this.lowerSize : this.upperSize;
  }
}
