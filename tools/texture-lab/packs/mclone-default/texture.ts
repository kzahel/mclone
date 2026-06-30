import { texturePack } from "../../src/dsl";
import { defineDirtTextures } from "./block/dirt";
import { defineGrassBlockTextures } from "./block/grass-block";

export default texturePack("mclone-default", (api) => {
  defineDirtTextures(api);
  defineGrassBlockTextures(api);
});
