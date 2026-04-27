import { LayerDefinitions } from "../layer-definitions";
import { LayerDefinition } from "./builders/layer-definition";
import { ModelLayerLocation } from "./model-layer-location";
import { ModelPart } from "./model-part";

export class EntityModelSet {
  private roots: ReadonlyMap<ModelLayerLocation, LayerDefinition>;

  public constructor(roots: ReadonlyMap<ModelLayerLocation, LayerDefinition> = new Map()) {
    this.roots = roots;
  }

  public bakeLayer(modelLayerLocation: ModelLayerLocation): ModelPart {
    const layerDefinition = this.roots.get(modelLayerLocation);
    if (layerDefinition === undefined) {
      throw new Error(`No model for layer ${modelLayerLocation.toString()}`);
    }

    return layerDefinition.bakeRoot();
  }

  public onResourceManagerReload(): void {
    // EntityRender0: reloads synthesize the currently ported roots instead of reading resource-pack model-layer data.
    this.roots = LayerDefinitions.createRoots();
  }

  public static createDefault(): EntityModelSet {
    return new EntityModelSet(LayerDefinitions.createRoots());
  }
}
