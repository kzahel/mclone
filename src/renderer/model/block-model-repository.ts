import { ResourceLocation } from "../../core/resource-location";
import { BLOCK_ENTITY_MARKER, BlockModel, GENERATION_MARKER } from "./block-model";
import { type UnbakedModel } from "./unbaked-model";

const BUILTIN_GENERATED = "builtin/generated";
const BUILTIN_ENTITY = "builtin/entity";
const BUILTIN_PREFIX = "builtin/";
const MISSING_MODEL_LOCATION = new ResourceLocation("builtin/missing");
const MISSING_MODEL_MESH = `{
  "textures": {
    "particle": "missingno",
    "missingno": "missingno"
  },
  "elements": [
    {
      "from": [0, 0, 0],
      "to": [16, 16, 16],
      "faces": {
        "down":  { "uv": [0, 0, 16, 16], "cullface": "down",  "texture": "#missingno" },
        "up":    { "uv": [0, 0, 16, 16], "cullface": "up",    "texture": "#missingno" },
        "north": { "uv": [0, 0, 16, 16], "cullface": "north", "texture": "#missingno" },
        "south": { "uv": [0, 0, 16, 16], "cullface": "south", "texture": "#missingno" },
        "west":  { "uv": [0, 0, 16, 16], "cullface": "west",  "texture": "#missingno" },
        "east":  { "uv": [0, 0, 16, 16], "cullface": "east",  "texture": "#missingno" }
      }
    }
  ]
}`;
const BUILTIN_MODELS = new Map<string, string>([["missing", MISSING_MODEL_MESH]]);

export interface BlockModelSource {
  getModelJson(location: ResourceLocation): string | undefined;
}

export class BlockModelRepository {
  private readonly cache = new Map<string, UnbakedModel>();

  public constructor(private readonly source: BlockModelSource) {}

  public getModel(location: ResourceLocation): UnbakedModel {
    const key = location.toString();
    const cached = this.cache.get(key);
    if (cached !== undefined) {
      return cached;
    }

    const model = this.loadBlockModel(location);
    this.cache.set(key, model);
    return model;
  }

  public getBlockModel(location: ResourceLocation): BlockModel {
    const model = this.getModel(location);
    if (!(model instanceof BlockModel)) {
      throw new Error(`Model ${location} is not a block model`);
    }

    return model;
  }

  public resolveBlockModel(location: ResourceLocation): BlockModel {
    const model = this.getBlockModel(location);
    this.resolveParents(location, model, new Set<string>());
    return model;
  }

  private resolveParents(location: ResourceLocation, model: BlockModel, resolving: Set<string>): void {
    if (model.getParentLocation() === undefined || model.getParent() !== undefined) {
      return;
    }

    const key = location.toString();
    if (resolving.has(key)) {
      model.setParentLocation(MISSING_MODEL_LOCATION);
      model.setParent(this.getBlockModel(MISSING_MODEL_LOCATION));
      return;
    }

    resolving.add(key);
    let parentLocation = model.getParentLocation()!;
    let parent: BlockModel;
    try {
      parent = this.getBlockModel(parentLocation);
      this.resolveParents(parentLocation, parent, resolving);
    } catch {
      parentLocation = MISSING_MODEL_LOCATION;
      parent = this.getBlockModel(parentLocation);
    } finally {
      resolving.delete(key);
    }

    model.setParentLocation(parentLocation);
    model.setParent(parent);
  }

  private loadBlockModel(location: ResourceLocation): BlockModel {
    const path = location.getPath();
    if (path === BUILTIN_GENERATED) {
      return GENERATION_MARKER;
    }

    if (path === BUILTIN_ENTITY) {
      return BLOCK_ENTITY_MARKER;
    }

    let modelJson: string | undefined;
    if (path.startsWith(BUILTIN_PREFIX)) {
      modelJson = BUILTIN_MODELS.get(path.substring(BUILTIN_PREFIX.length));
      if (modelJson === undefined) {
        throw new Error(location.toString());
      }
    } else {
      modelJson = this.source.getModelJson(location);
      if (modelJson === undefined) {
        throw new Error(`Missing model ${location}`);
      }
    }

    const model = BlockModel.fromString(modelJson);
    model.name = location.toString();
    return model;
  }
}
