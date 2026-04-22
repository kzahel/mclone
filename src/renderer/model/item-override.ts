import { ResourceLocation } from "../../core/resource-location";
import { expectJsonObject, getAsJsonObject, getAsString, type JsonObject } from "./model-json-utils";

export class ItemOverridePredicate {
  public constructor(
    private readonly property: ResourceLocation,
    private readonly value: number,
  ) {}

  public getProperty(): ResourceLocation {
    return this.property;
  }

  public getValue(): number {
    return this.value;
  }
}

export class ItemOverride {
  public constructor(
    private readonly model: ResourceLocation,
    private readonly predicates: readonly ItemOverridePredicate[],
  ) {}

  public getModel(): ResourceLocation {
    return this.model;
  }

  public getPredicates(): readonly ItemOverridePredicate[] {
    return this.predicates;
  }

  public static fromJson(value: unknown): ItemOverride {
    const json = expectJsonObject(value, "override");
    const model = new ResourceLocation(getAsString(json, "model"));
    const predicates = ItemOverride.getPredicates(json);
    return new ItemOverride(model, predicates);
  }

  private static getPredicates(json: JsonObject): readonly ItemOverridePredicate[] {
    const predicatesJson = getAsJsonObject(json, "predicate");
    return Object.entries(predicatesJson).map(([name, value]) => new ItemOverridePredicate(new ResourceLocation(name), Number(value)));
  }
}
