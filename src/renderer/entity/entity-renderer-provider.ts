import { EntityModelSet } from "../model/geom/entity-model-set";
import type { ModelLayerLocation } from "../model/geom/model-layer-location";
import type { ModelPart } from "../model/geom/model-part";
import type { EntityRenderDispatcher } from "./entity-render-dispatcher";

export namespace EntityRendererProvider {
  export class Context {
    public constructor(
      private readonly entityRenderDispatcher: EntityRenderDispatcher,
      private readonly modelSet: EntityModelSet,
    ) {}

    public getEntityRenderDispatcher(): EntityRenderDispatcher {
      return this.entityRenderDispatcher;
    }

    public getModelSet(): EntityModelSet {
      return this.modelSet;
    }

    public bakeLayer(modelLayerLocation: ModelLayerLocation): ModelPart {
      return this.modelSet.bakeLayer(modelLayerLocation);
    }
  }
}
