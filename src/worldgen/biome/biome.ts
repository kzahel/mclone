import type { NoiseBiome } from "./noise-biome";

export interface BiomeDefinition {
  id: number;
  key: string;
  depth: number;
  scale: number;
}

export class Biome implements NoiseBiome {
  public constructor(
    private readonly id: number,
    private readonly key: string,
    private readonly depth: number,
    private readonly scale: number,
  ) {}

  public getId(): number {
    return this.id;
  }

  public getKey(): string {
    return this.key;
  }

  public getDepth(): number {
    return this.depth;
  }

  public getScale(): number {
    return this.scale;
  }
}
