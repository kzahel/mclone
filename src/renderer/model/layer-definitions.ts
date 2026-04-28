import { ModelLayerLocation } from "./geom/model-layer-location";
import { ModelLayers } from "./geom/model-layers";
import { CubeDeformation } from "./geom/builders/cube-deformation";
import { LayerDefinition } from "./geom/builders/layer-definition";
import { CowModel } from "./cow-model";
import { PigModel } from "./pig-model";
import { PlayerModel } from "./player-model";

export class LayerDefinitions {
  public static createRoots(): ReadonlyMap<ModelLayerLocation, LayerDefinition> {
    const roots = new Map<ModelLayerLocation, LayerDefinition>();
    roots.set(ModelLayers.COW, CowModel.createBodyLayer());
    roots.set(ModelLayers.PIG, PigModel.createBodyLayer(CubeDeformation.NONE));
    roots.set(ModelLayers.PLAYER, LayerDefinition.create(PlayerModel.createMesh(CubeDeformation.NONE, false), 64, 64));
    roots.set(ModelLayers.PLAYER_SLIM, LayerDefinition.create(PlayerModel.createMesh(CubeDeformation.NONE, true), 64, 64));
    return roots;
  }
}
