import { texturePack } from "../../src/dsl";
import { defineDirtTextures } from "./block/dirt";
import { defineFarLodTerrainMaterialTextures } from "./block/far-lod-materials";
import { defineGrassBlockTextures } from "./block/grass-block";
import { defineStoneTextures } from "./block/stone";

export default texturePack("mclone-default", (api) => {
  defineDirtTextures(api);
  defineGrassBlockTextures(api);
  defineStoneTextures(api);
  defineFarLodTerrainMaterialTextures(api);
});
