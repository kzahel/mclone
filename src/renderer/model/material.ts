import { ResourceLocation } from "../../core/resource-location";

export class Material {
  public constructor(
    private readonly atlasLocationValue: ResourceLocation,
    private readonly textureValue: ResourceLocation,
  ) {}

  public atlasLocation(): ResourceLocation {
    return this.atlasLocationValue;
  }

  public texture(): ResourceLocation {
    return this.textureValue;
  }

  public equals(other: unknown): boolean {
    return (
      other instanceof Material &&
      this.atlasLocationValue.equals(other.atlasLocationValue) &&
      this.textureValue.equals(other.textureValue)
    );
  }

  public toString(): string {
    return `Material{atlasLocation=${this.atlasLocationValue}, texture=${this.textureValue}}`;
  }
}
