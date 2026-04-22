import { expectJsonObject, getAsJsonArray, getAsJsonObject, hasJsonValue, type JsonObject } from "../model-json-utils";
import { MultiVariant } from "../multi-variant";
import { AndCondition } from "./and-condition";
import { type Condition, TRUE_CONDITION } from "./condition";
import { KeyValueCondition } from "./key-value-condition";
import { OrCondition } from "./or-condition";

export class Selector {
  public constructor(
    private readonly condition: Condition,
    private readonly variant: MultiVariant,
  ) {
    if (condition === undefined || variant === undefined) {
      throw new Error("Missing condition or variant for selector");
    }
  }

  public static fromJson(value: unknown): Selector {
    const json = expectJsonObject(value, "selector");
    const condition = hasJsonValue(json, "when") ? Selector.getCondition(getAsJsonObject(json, "when")) : TRUE_CONDITION;
    return new Selector(condition, MultiVariant.fromJson(json.apply));
  }

  public getVariant(): MultiVariant {
    return this.variant;
  }

  public getPredicate(definition: import("../../../world/level/block/state/state-definition").StateDefinition<
    import("../../../world/level/block/block").Block,
    import("../../../world/level/block/state/block-state").BlockState
  >): (state: import("../../../world/level/block/state/block-state").BlockState) => boolean {
    return this.condition.getPredicate(definition);
  }

  public static getCondition(json: JsonObject): Condition {
    const entries = Object.entries(json);
    if (entries.length === 0) {
      throw new Error("No elements found in selector");
    }

    if (entries.length === 1) {
      if (hasJsonValue(json, OrCondition.TOKEN)) {
        return new OrCondition(getAsJsonArray(json, OrCondition.TOKEN).map((entry) => Selector.getCondition(expectJsonObject(entry, "OR selector"))));
      }

      if (hasJsonValue(json, AndCondition.TOKEN)) {
        return new AndCondition(getAsJsonArray(json, AndCondition.TOKEN).map((entry) => Selector.getCondition(expectJsonObject(entry, "AND selector"))));
      }

      return Selector.getKeyValueCondition(entries[0]!);
    }

    return new AndCondition(entries.map((entry) => Selector.getKeyValueCondition(entry)));
  }

  private static getKeyValueCondition([key, value]: readonly [string, unknown]): Condition {
    if (typeof value !== "string") {
      throw new Error(`Expected selector value for '${key}' to be a string`);
    }

    return new KeyValueCondition(key, value);
  }
}
