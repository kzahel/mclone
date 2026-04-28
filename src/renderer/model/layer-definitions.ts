import { ModelLayerLocation } from "./geom/model-layer-location";
import { ModelLayers } from "./geom/model-layers";
import { CubeDeformation } from "./geom/builders/cube-deformation";
import { LayerDefinition } from "./geom/builders/layer-definition";
import { ChickenModel } from "./chicken-model";
import { CowModel } from "./cow-model";
import { PigModel } from "./pig-model";
import { PlayerModel } from "./player-model";
import { RabbitModel } from "./rabbit-model";
import { SheepFurModel } from "./sheep-fur-model";
import { SheepModel } from "./sheep-model";
import { WolfModel } from "./wolf-model";

export class LayerDefinitions {
  public static createRoots(): ReadonlyMap<ModelLayerLocation, LayerDefinition> {
    const roots = new Map<ModelLayerLocation, LayerDefinition>();
    roots.set(ModelLayers.CHICKEN, ChickenModel.createBodyLayer());
    roots.set(ModelLayers.COW, CowModel.createBodyLayer());
    roots.set(ModelLayers.MOOSHROOM, CowModel.createBodyLayer());
    roots.set(ModelLayers.PIG, PigModel.createBodyLayer(CubeDeformation.NONE));
    roots.set(ModelLayers.PLAYER, LayerDefinition.create(PlayerModel.createMesh(CubeDeformation.NONE, false), 64, 64));
    roots.set(ModelLayers.PLAYER_SLIM, LayerDefinition.create(PlayerModel.createMesh(CubeDeformation.NONE, true), 64, 64));
    roots.set(ModelLayers.RABBIT, RabbitModel.createBodyLayer());
    roots.set(ModelLayers.SHEEP, SheepModel.createBodyLayer());
    roots.set(ModelLayers.SHEEP_FUR, SheepFurModel.createFurLayer());
    roots.set(ModelLayers.WOLF, WolfModel.createBodyLayer());
    return roots;
  }
}
