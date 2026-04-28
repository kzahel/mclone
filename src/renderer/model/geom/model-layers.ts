import { ResourceLocation } from "../../../core/resource-location";
import { ModelLayerLocation } from "./model-layer-location";

const ALL_MODELS = new Map<string, ModelLayerLocation>();

function register(path: string, model = "main"): ModelLayerLocation {
  const location = new ModelLayerLocation(new ResourceLocation("minecraft", path), model);
  const key = location.hashCode();
  if (ALL_MODELS.has(key)) {
    throw new Error(`Duplicate registration for ${location.toString()}`);
  }

  ALL_MODELS.set(key, location);
  return location;
}

export class ModelLayers {
  public static readonly CHICKEN = register("chicken");
  public static readonly COW = register("cow");
  public static readonly MOOSHROOM = register("mooshroom");
  public static readonly PIG = register("pig");
  public static readonly PLAYER = register("player");
  public static readonly PLAYER_SLIM = register("player_slim");
  public static readonly RABBIT = register("rabbit");
  public static readonly SHEEP = register("sheep");
  public static readonly SHEEP_FUR = register("sheep", "fur");
  public static readonly WOLF = register("wolf");

  public static getKnownLocations(): readonly ModelLayerLocation[] {
    return [...ALL_MODELS.values()];
  }
}
