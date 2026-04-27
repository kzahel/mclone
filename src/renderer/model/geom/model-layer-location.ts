import { ResourceLocation } from "../../../core/resource-location";

export class ModelLayerLocation {
  public constructor(
    private readonly model: ResourceLocation,
    private readonly layer: string,
  ) {}

  public getModel(): ResourceLocation {
    return this.model;
  }

  public getLayer(): string {
    return this.layer;
  }

  public equals(other: unknown): boolean {
    return other instanceof ModelLayerLocation && this.model.equals(other.model) && this.layer === other.layer;
  }

  public hashCode(): string {
    return `${this.model.hashCode()}#${this.layer}`;
  }

  public toString(): string {
    return `${this.model.toString()}#${this.layer}`;
  }
}
