import { texturePack } from "../../src/dsl";
import { defineDirtTextures } from "./block/dirt";
import { defineDirectionalCubeTextures } from "./block/directional-cubes";
import { defineFarLodTerrainMaterialTextures } from "./block/far-lod-materials";
import { defineGrassBlockTextures } from "./block/grass-block";
import { defineStoneTextures } from "./block/stone";

export default texturePack("mclone-default", (api) => {
  defineDirtTextures(api);
  defineGrassBlockTextures(api);
  defineStoneTextures(api);
  defineDirectionalCubeTextures(api);
  defineFarLodTerrainMaterialTextures(api);
});
