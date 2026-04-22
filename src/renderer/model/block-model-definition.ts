import type { Block } from "../../world/level/block/block";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { StateDefinition } from "../../world/level/block/state/state-definition";
import { expectJsonObject, getAsJsonObject, hasJsonValue, type JsonObject } from "./model-json-utils";
import { MultiVariant } from "./multi-variant";
import { MultiPart } from "./multipart/multi-part";

export class BlockModelDefinition {
  private readonly variants = new Map<string, MultiVariant>();
  private readonly multiPartValue: MultiPart | undefined;

  public constructor(variants: ReadonlyMap<string, MultiVariant>, multiPart?: MultiPart) {
    for (const [key, value] of variants.entries()) {
      this.variants.set(key, value);
    }

    this.multiPartValue = multiPart;
  }

  public static fromString(context: BlockModelDefinition.Context, value: string): BlockModelDefinition {
    return BlockModelDefinition.fromJson(context, JSON.parse(value));
  }

  public static fromJson(context: BlockModelDefinition.Context, value: unknown): BlockModelDefinition {
    const json = expectJsonObject(value, "block model definition");
    const variants = BlockModelDefinition.getVariants(json);
    const multiPart = BlockModelDefinition.getMultiPart(context, json);
    if (variants.size === 0 && (multiPart === undefined || multiPart.getMultiVariants().size === 0)) {
      throw new Error("Neither 'variants' nor 'multipart' found");
    }

    return new BlockModelDefinition(variants, multiPart);
  }

  public hasVariant(name: string): boolean {
    return this.variants.has(name);
  }

  public getVariant(name: string): MultiVariant {
    const variant = this.variants.get(name);
    if (variant === undefined) {
      throw new MissingVariantException();
    }

    return variant;
  }

  public getVariants(): ReadonlyMap<string, MultiVariant> {
    return this.variants;
  }

  public getMultiVariants(): Set<MultiVariant> {
    const result = new Set<MultiVariant>(this.variants.values());
    if (this.isMultiPart()) {
      for (const variant of this.multiPartValue!.getMultiVariants()) {
        result.add(variant);
      }
    }

    return result;
  }

  public isMultiPart(): boolean {
    return this.multiPartValue !== undefined;
  }

  public getMultiPart(): MultiPart {
    if (this.multiPartValue === undefined) {
      throw new Error("Block model definition is not multipart");
    }

    return this.multiPartValue;
  }

  private static getVariants(json: JsonObject): Map<string, MultiVariant> {
    const variants = new Map<string, MultiVariant>();
    if (!hasJsonValue(json, "variants")) {
      return variants;
    }

    const variantsJson = getAsJsonObject(json, "variants");
    for (const [key, value] of Object.entries(variantsJson)) {
      variants.set(key, MultiVariant.fromJson(value));
    }

    return variants;
  }

  private static getMultiPart(context: BlockModelDefinition.Context, json: JsonObject): MultiPart | undefined {
    if (!hasJsonValue(json, "multipart")) {
      return undefined;
    }

    return MultiPart.fromJson(context.getDefinition(), json.multipart);
  }
}

export namespace BlockModelDefinition {
  export class Context {
    private definition: StateDefinition<Block, BlockState> | undefined;

    public getDefinition(): StateDefinition<Block, BlockState> {
      if (this.definition === undefined) {
        throw new Error("BlockModelDefinition.Context is missing a StateDefinition");
      }

      return this.definition;
    }

    public setDefinition(definition: StateDefinition<Block, BlockState>): void {
      this.definition = definition;
    }
  }
}

export class MissingVariantException extends Error {}
