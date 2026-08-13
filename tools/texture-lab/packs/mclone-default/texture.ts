import { texturePack } from "../../src/dsl";
import { defineDirtTextures } from "./block/dirt";
import { defineDirectionalCubeTextures } from "./block/directional-cubes";
import { defineFarmsteadMaterialTextures } from "./block/farmstead-materials";
import { defineGrassBlockTextures } from "./block/grass-block";
import { definePartialShapeTextures } from "./block/partial-shapes";
import { definePlantAndFlatTextures } from "./block/plants-and-flats";
import { defineStoneTextures } from "./block/stone";
import { defineWheatFarmingTextures } from "./block/wheat-farming";

export default texturePack("mclone-default", (api) => {
  defineDirtTextures(api);
  defineGrassBlockTextures(api);
  defineStoneTextures(api);
  defineDirectionalCubeTextures(api);
  definePlantAndFlatTextures(api);
  definePartialShapeTextures(api);
  defineFarmsteadMaterialTextures(api);
  defineWheatFarmingTextures(api);
});
