import path from "node:path";
import { assetLabRoot, repositoryRoot } from "./vite-figure-path";

export interface FirstPartyFigure {
  name: string;
  outputPath: string;
  sourcePath: string;
}

export const FIRST_PARTY_FIGURES: readonly FirstPartyFigure[] = [
  firstPartyFigure("player"),
  firstPartyFigure("chicken"),
  firstPartyFigure("upright_bear"),
];

function firstPartyFigure(name: string): FirstPartyFigure {
  return {
    name,
    sourcePath: path.join(assetLabRoot, "examples", name, "figure.ts"),
    outputPath: path.join(repositoryRoot, "assets", "mclone", "figures", `${name}.figure.json`),
  };
}
