export enum LightLayer {
  SKY = "sky",
  BLOCK = "block",
}

export function getLightLayerSurrounding(layer: LightLayer): number {
  switch (layer) {
    case LightLayer.SKY:
      return 15;
    case LightLayer.BLOCK:
      return 0;
  }
}
