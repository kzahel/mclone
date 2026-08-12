import path from "node:path";
import {
  semanticAssetAnchorForUse,
  type SemanticAssetAnchor,
  type SemanticInstantiationStatus,
  type SemanticAssetUse,
} from "./semantic-assets";
import { assetLabRoot, repositoryRoot } from "./vite-figure-path";

export interface FirstPartySemanticAsset {
  anchor: SemanticAssetAnchor;
  name: string;
  instantiation: SemanticInstantiationStatus;
  outputPath: string;
  runtimeAssetId: string;
  runtimePath: string;
  sourcePath: string;
  use: SemanticAssetUse;
}

export type FirstPartyFigure = FirstPartySemanticAsset & { anchor: "feet"; use: "actor" };

export const FIRST_PARTY_SEMANTIC_ASSETS: readonly FirstPartySemanticAsset[] = [
  firstPartyActor("player"),
  firstPartyActor("cow"),
  firstPartyActor("chicken"),
  firstPartyActor("mallard_duck"),
  firstPartyActor("upright_bear"),
  firstPartyActor("deer"),
  firstPartyActor("bee", "review_only"),
  firstPartyProp("mallard_nest", "world_prop", "ground"),
  firstPartyProp("mallard_feather", "item_prop", "item_center"),
  firstPartyProp("hunting_spear", "item_prop", "item_center"),
  firstPartyProp("venison", "item_prop", "item_center"),
  firstPartyProp("deer_hide", "item_prop", "item_center"),
  firstPartyProp("shed_antler", "item_prop", "item_center"),
  firstPartyProp("deer_bed", "world_prop", "ground"),
  firstPartyProp("bee_nest", "world_prop", "ground", "review_only"),
  firstPartyProp("bee_hotel", "world_prop", "ground", "review_only"),
  firstPartyProp("bee_hotel_item", "item_prop", "item_center", "review_only"),
  firstPartyProp("beeswax", "item_prop", "item_center", "review_only"),
];

export const FIRST_PARTY_FIGURES: readonly FirstPartyFigure[] =
  FIRST_PARTY_SEMANTIC_ASSETS.filter(isFirstPartyFigure);

function firstPartyActor(
  name: string,
  instantiation: SemanticInstantiationStatus = "live_gameplay",
): FirstPartyFigure {
  const runtimePath = `assets/mclone/figures/${name}.figure.json`;
  return {
    anchor: "feet",
    instantiation,
    name,
    sourcePath: path.join(assetLabRoot, "examples", name, "figure.ts"),
    outputPath: path.join(repositoryRoot, runtimePath),
    runtimeAssetId: `mclone:${name}`,
    runtimePath,
    use: "actor",
  };
}

function firstPartyProp(
  name: string,
  use: Exclude<SemanticAssetUse, "actor">,
  anchor: Exclude<SemanticAssetAnchor, "feet">,
  instantiation: SemanticInstantiationStatus = "live_gameplay",
): FirstPartySemanticAsset {
  if (semanticAssetAnchorForUse(use) !== anchor) {
    throw new Error(`Semantic asset '${name}' has incompatible use '${use}' and anchor '${anchor}'`);
  }
  const runtimePath = `assets/mclone/figures/${name}.figure.json`;
  return {
    anchor,
    instantiation,
    name,
    sourcePath: path.join(assetLabRoot, "props", name, "figure.ts"),
    outputPath: path.join(repositoryRoot, runtimePath),
    runtimeAssetId: `mclone:${name}`,
    runtimePath,
    use,
  };
}

function isFirstPartyFigure(asset: FirstPartySemanticAsset): asset is FirstPartyFigure {
  return asset.use === "actor" && asset.anchor === "feet";
}
