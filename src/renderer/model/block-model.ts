import { ResourceLocation } from "../../core/resource-location";
import { MissingTextureAtlasSprite } from "../texture/missing-texture-atlas-sprite";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";
import { TextureAtlas } from "../texture/texture-atlas";
import { type BakedModel } from "./baked-model";
import { BlockElement } from "./block-element";
import { BuiltInModel } from "./built-in-model";
import { FaceBakery, rotateDirection } from "./face-bakery";
import { ItemOverrides } from "./item-overrides";
import { expectJsonObject, getAsBoolean, getAsJsonObject, getAsString, hasJsonValue, type JsonObject } from "./model-json-utils";
import { type ModelBakery } from "./model-bakery";
import { type ModelState } from "./model-state";
import { ItemOverride } from "./item-override";
import { ItemTransform } from "./item-transform";
import { ItemTransforms, TransformType } from "./item-transforms";
import { Material } from "./material";
import { SimpleBakedModel } from "./simple-baked-model";
import { type UnbakedModel } from "./unbaked-model";

type TextureReference = { readonly kind: "reference"; readonly value: string };
type TextureMaterial = { readonly kind: "material"; readonly value: Material };
type TextureSlot = TextureReference | TextureMaterial;

function textureReference(value: string): TextureReference {
  return { kind: "reference", value };
}

function textureMaterial(value: Material): TextureMaterial {
  return { kind: "material", value };
}

function missingMaterial(): Material {
  return new Material(TextureAtlas.LOCATION_BLOCKS, MissingTextureAtlasSprite.getLocation());
}

function parseTextureLocationOrReference(atlasLocation: ResourceLocation, value: string): TextureSlot {
  if (BlockModel.isTextureReference(value)) {
    return textureReference(value.substring(1));
  }

  const location = ResourceLocation.tryParse(value);
  if (location === undefined) {
    throw new Error(`${value} is not valid resource location`);
  }

  return textureMaterial(new Material(atlasLocation, location));
}

export enum GuiLight {
  FRONT = "front",
  SIDE = "side",
}

export function getGuiLightByName(name: string): GuiLight {
  switch (name) {
    case GuiLight.FRONT:
      return GuiLight.FRONT;
    case GuiLight.SIDE:
      return GuiLight.SIDE;
    default:
      throw new Error(`Invalid gui light: ${name}`);
  }
}

export function guiLightLikeBlock(guiLight: GuiLight): boolean {
  return guiLight === GuiLight.SIDE;
}

export class BlockModel implements UnbakedModel {
  private static readonly FACE_BAKERY = new FaceBakery();
  public name = "";

  protected parent: BlockModel | undefined;
  protected parentLocation: ResourceLocation | undefined;

  public constructor(
    private readonly elements: readonly BlockElement[],
    private readonly textureMap: ReadonlyMap<string, TextureSlot>,
    private readonly hasAmbientOcclusionValue: boolean,
    private readonly guiLightValue: GuiLight | undefined,
    private readonly transforms: ItemTransforms,
    private readonly overrides: readonly ItemOverride[],
    parentLocation?: ResourceLocation,
  ) {
    this.parentLocation = parentLocation;
  }

  public static fromString(value: string): BlockModel {
    return BlockModel.fromJson(JSON.parse(value));
  }

  public static fromJson(value: unknown): BlockModel {
    const json = expectJsonObject(value, "block model");
    const elements = BlockModel.getElements(json);
    const parentName = BlockModel.getParentName(json);
    const textureMap = BlockModel.getTextureMap(json);
    const ambientOcclusion = BlockModel.getAmbientOcclusion(json);
    let transforms = ItemTransforms.NO_TRANSFORMS;
    if (hasJsonValue(json, "display")) {
      transforms = ItemTransforms.fromJson(getAsJsonObject(json, "display"));
    }

    const overrides = BlockModel.getOverrides(json);
    let guiLight: GuiLight | undefined;
    if (hasJsonValue(json, "gui_light")) {
      guiLight = getGuiLightByName(getAsString(json, "gui_light"));
    }

    const parentLocation = parentName.length === 0 ? undefined : new ResourceLocation(parentName);
    return new BlockModel(elements, textureMap, ambientOcclusion, guiLight, transforms, overrides, parentLocation);
  }

  public getElements(): readonly BlockElement[] {
    return this.elements.length === 0 && this.parent ? this.parent.getElements() : this.elements;
  }

  public hasAmbientOcclusion(): boolean {
    return this.parent ? this.parent.hasAmbientOcclusion() : this.hasAmbientOcclusionValue;
  }

  public getGuiLight(): GuiLight {
    if (this.guiLightValue !== undefined) {
      return this.guiLightValue;
    }

    return this.parent ? this.parent.getGuiLight() : GuiLight.SIDE;
  }

  public isResolved(): boolean {
    return this.parentLocation === undefined || (this.parent !== undefined && this.parent.isResolved());
  }

  public getOverrides(): readonly ItemOverride[] {
    return this.overrides;
  }

  public getDependencies(): readonly ResourceLocation[] {
    const dependencies: ResourceLocation[] = [];
    for (const override of this.overrides) {
      dependencies.push(override.getModel());
    }

    if (this.parentLocation !== undefined) {
      dependencies.push(this.parentLocation);
    }

    return dependencies;
  }

  private getItemOverrides(_bakery: ModelBakery, _model: BlockModel): ItemOverrides {
    return ItemOverrides.EMPTY;
  }

  public bake(
    bakery: ModelBakery,
    spriteGetter: (material: Material) => TextureAtlasSprite,
    modelState: ModelState,
    location: ResourceLocation,
  ): BakedModel {
    return this.bakeModel(bakery, this, spriteGetter, modelState, location, true);
  }

  public bakeModel(
    bakery: ModelBakery,
    model: BlockModel,
    spriteGetter: (material: Material) => TextureAtlasSprite,
    modelState: ModelState,
    _location: ResourceLocation,
    isGui3d: boolean,
  ): BakedModel {
    const particle = spriteGetter(this.getMaterial("particle"));
    if (bakery.isBlockEntityMarker(this)) {
      return new BuiltInModel(this.getTransforms(), this.getItemOverrides(bakery, model), particle, guiLightLikeBlock(this.getGuiLight()));
    }

    const builder = new SimpleBakedModel.Builder(this, this.getItemOverrides(bakery, model), isGui3d).particle(particle);
    for (const element of this.getElements()) {
      for (const [direction, elementFace] of element.faces.entries()) {
        const sprite = spriteGetter(this.getMaterial(elementFace.texture));
        const quad = BlockModel.FACE_BAKERY.bakeQuad(
          element.from,
          element.to,
          elementFace,
          sprite,
          direction,
          modelState,
          element.rotation,
          element.shade,
          _location,
        );
        if (elementFace.cullForDirection === undefined) {
          builder.addUnculledFace(quad);
        } else {
          builder.addCulledFace(rotateDirection(modelState.getRotation().getMatrix(), elementFace.cullForDirection), quad);
        }
      }
    }

    return builder.build();
  }

  public hasTexture(name: string): boolean {
    return !missingMaterial().texture().equals(this.getMaterial(name).texture());
  }

  public getMaterial(name: string): Material {
    if (BlockModel.isTextureReference(name)) {
      name = name.substring(1);
    }

    const referenceChain: string[] = [];
    while (true) {
      const texture = this.findTextureEntry(name);
      if (texture.kind === "material") {
        return texture.value;
      }

      name = texture.value;
      if (referenceChain.includes(name)) {
        return missingMaterial();
      }

      referenceChain.push(name);
    }
  }

  private findTextureEntry(name: string): TextureSlot {
    for (let model: BlockModel | undefined = this; model !== undefined; model = model.parent) {
      const texture = model.textureMap.get(name);
      if (texture !== undefined) {
        return texture;
      }
    }

    return textureMaterial(missingMaterial());
  }

  public static isTextureReference(value: string): boolean {
    return value.charAt(0) === "#";
  }

  public getRootModel(): BlockModel {
    return this.parent === undefined ? this : this.parent.getRootModel();
  }

  public getTransforms(): ItemTransforms {
    const thirdPersonLeftHand = this.getTransform(TransformType.THIRD_PERSON_LEFT_HAND);
    const thirdPersonRightHand = this.getTransform(TransformType.THIRD_PERSON_RIGHT_HAND);
    const firstPersonLeftHand = this.getTransform(TransformType.FIRST_PERSON_LEFT_HAND);
    const firstPersonRightHand = this.getTransform(TransformType.FIRST_PERSON_RIGHT_HAND);
    const head = this.getTransform(TransformType.HEAD);
    const gui = this.getTransform(TransformType.GUI);
    const ground = this.getTransform(TransformType.GROUND);
    const fixed = this.getTransform(TransformType.FIXED);
    return new ItemTransforms(thirdPersonLeftHand, thirdPersonRightHand, firstPersonLeftHand, firstPersonRightHand, head, gui, ground, fixed);
  }

  private getTransform(type: TransformType): ItemTransform {
    return this.parent !== undefined && !this.transforms.hasTransform(type) ? this.parent.getTransform(type) : this.transforms.getTransform(type);
  }

  public getParentLocation(): ResourceLocation | undefined {
    return this.parentLocation;
  }

  public getParent(): BlockModel | undefined {
    return this.parent;
  }

  public setParent(parent: BlockModel | undefined): void {
    this.parent = parent;
  }

  public setParentLocation(parentLocation: ResourceLocation | undefined): void {
    this.parentLocation = parentLocation;
  }

  public toString(): string {
    return this.name;
  }

  private static getOverrides(json: JsonObject): readonly ItemOverride[] {
    if (!hasJsonValue(json, "overrides")) {
      return [];
    }

    const overridesJson = json.overrides;
    if (!Array.isArray(overridesJson)) {
      throw new Error("Expected overrides to be an array");
    }

    return overridesJson.map((entry) => ItemOverride.fromJson(entry));
  }

  private static getTextureMap(json: JsonObject): ReadonlyMap<string, TextureSlot> {
    const atlasLocation = TextureAtlas.LOCATION_BLOCKS;
    const textureMap = new Map<string, TextureSlot>();
    if (!hasJsonValue(json, "textures")) {
      return textureMap;
    }

    const texturesJson = getAsJsonObject(json, "textures");
    for (const [key, value] of Object.entries(texturesJson)) {
      if (typeof value !== "string") {
        throw new Error(`Expected textures.${key} to be a string`);
      }

      textureMap.set(key, parseTextureLocationOrReference(atlasLocation, value));
    }

    return textureMap;
  }

  private static getParentName(json: JsonObject): string {
    return getAsString(json, "parent", "");
  }

  private static getAmbientOcclusion(json: JsonObject): boolean {
    return getAsBoolean(json, "ambientocclusion", true);
  }

  private static getElements(json: JsonObject): readonly BlockElement[] {
    if (!hasJsonValue(json, "elements")) {
      return [];
    }

    const elementsJson = json.elements;
    if (!Array.isArray(elementsJson)) {
      throw new Error("Expected elements to be an array");
    }

    return elementsJson.map((entry) => BlockElement.fromJson(entry));
  }
}

export const GENERATION_MARKER = BlockModel.fromString("{\"gui_light\": \"front\"}");
GENERATION_MARKER.name = "generation marker";

export const BLOCK_ENTITY_MARKER = BlockModel.fromString("{\"gui_light\": \"side\"}");
BLOCK_ENTITY_MARKER.name = "block entity marker";

export const ITEM_MODEL_LAYERS = ["layer0", "layer1", "layer2", "layer3", "layer4"] as const;

export class LoopException extends Error {}
