import { ModelLayerLocation } from "./geom/model-layer-location";
import { ModelLayers } from "./geom/model-layers";
import { CubeDeformation } from "./geom/builders/cube-deformation";
import { LayerDefinition } from "./geom/builders/layer-definition";
import { PlayerModel } from "./player-model";

export class LayerDefinitions {
  public static createRoots(): ReadonlyMap<ModelLayerLocation, LayerDefinition> {
    const roots = new Map<ModelLayerLocation, LayerDefinition>();
    // EntityRender0: only player roots are registered until additional entity renderers are ported.
    roots.set(ModelLayers.PLAYER, LayerDefinition.create(PlayerModel.createMesh(CubeDeformation.NONE, false), 64, 64));
    roots.set(ModelLayers.PLAYER_SLIM, LayerDefinition.create(PlayerModel.createMesh(CubeDeformation.NONE, true), 64, 64));
    return roots;
  }
}
