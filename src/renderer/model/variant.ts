import { ResourceLocation } from "../../core/resource-location";
import { BlockModelRotation } from "./block-model-rotation";
import { expectJsonObject, getAsBoolean, getAsNumber, getAsString } from "./model-json-utils";
import { type ModelState } from "./model-state";
import { Transformation } from "./transformation";

export class Variant implements ModelState {
  public constructor(
    private readonly modelLocation: ResourceLocation,
    private readonly rotation: Transformation,
    private readonly uvLock: boolean,
    private readonly weight: number,
  ) {}

  public static fromJson(value: unknown): Variant {
    const json = expectJsonObject(value, "variant");
    const modelLocation = new ResourceLocation(getAsString(json, "model"));
    const x = getAsNumber(json, "x", 0);
    const y = getAsNumber(json, "y", 0);
    const blockRotation = BlockModelRotation.by(x, y);
    if (blockRotation === undefined) {
      throw new Error(`Invalid BlockModelRotation x: ${x}, y: ${y}`);
    }

    const uvLock = getAsBoolean(json, "uvlock", false);
    const weight = getAsNumber(json, "weight", 1);
    if (weight < 1) {
      throw new Error(`Invalid weight ${weight} found, expected integer >= 1`);
    }

    return new Variant(modelLocation, blockRotation.getRotation(), uvLock, weight);
  }

  public getModelLocation(): ResourceLocation {
    return this.modelLocation;
  }

  public getRotation(): Transformation {
    return this.rotation;
  }

  public isUvLocked(): boolean {
    return this.uvLock;
  }

  public getWeight(): number {
    return this.weight;
  }

  public toString(): string {
    return `Variant{modelLocation=${this.modelLocation}, rotation=${this.rotation}, uvLock=${this.uvLock}, weight=${this.weight}}`;
  }
}
