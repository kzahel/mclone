export interface NoiseModifier {
  modifyNoise(noise: number, y: number, z: number, x: number): number;
}

export const NoiseModifier = {
  PASSTHROUGH: {
    modifyNoise(noise: number): number {
      return noise;
    },
  } satisfies NoiseModifier,
};
