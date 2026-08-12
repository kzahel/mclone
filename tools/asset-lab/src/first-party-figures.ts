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
  firstPartyProp("mallard_nest", "world_prop", "ground"),
  firstPartyProp("mallard_feather", "item_prop", "item_center"),
  firstPartyProp("mallard_tracks", "trace_prop", "surface_trace"),
];

export const FIRST_PARTY_FIGURES: readonly FirstPartyFigure[] =
  FIRST_PARTY_SEMANTIC_ASSETS.filter(isFirstPartyFigure);

function firstPartyActor(name: string): FirstPartyFigure {
  const runtimePath = `assets/mclone/figures/${name}.figure.json`;
  return {
    anchor: "feet",
    instantiation: "live_gameplay",
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
): FirstPartySemanticAsset {
  if (semanticAssetAnchorForUse(use) !== anchor) {
    throw new Error(`Semantic asset '${name}' has incompatible use '${use}' and anchor '${anchor}'`);
  }
  const runtimePath = `assets/mclone/figures/${name}.figure.json`;
  return {
    anchor,
    instantiation: "review_only",
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
