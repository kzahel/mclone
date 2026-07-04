import { texturePack } from "../../src/dsl";
import { defineDirtTextures } from "./block/dirt";
import { defineDirectionalCubeTextures } from "./block/directional-cubes";
import { defineFarLodTerrainMaterialTextures } from "./block/far-lod-materials";
import { defineGrassBlockTextures } from "./block/grass-block";
import { definePlantAndFlatTextures } from "./block/plants-and-flats";
import { defineStoneTextures } from "./block/stone";

export default texturePack("mclone-default", (api) => {
  defineDirtTextures(api);
  defineGrassBlockTextures(api);
  defineStoneTextures(api);
  defineDirectionalCubeTextures(api);
  definePlantAndFlatTextures(api);
  defineFarLodTerrainMaterialTextures(api);
});
