import path from "node:path";
import { assetLabRoot, repositoryRoot } from "./vite-figure-path";

export interface FirstPartyFigure {
  name: string;
  outputPath: string;
  runtimeFigureId: string;
  runtimePath: string;
  sourcePath: string;
}

export const FIRST_PARTY_FIGURES: readonly FirstPartyFigure[] = [
  firstPartyFigure("player"),
  firstPartyFigure("chicken"),
  firstPartyFigure("upright_bear"),
];

function firstPartyFigure(name: string): FirstPartyFigure {
  const runtimePath = `assets/mclone/figures/${name}.figure.json`;
  return {
    name,
    sourcePath: path.join(assetLabRoot, "examples", name, "figure.ts"),
    outputPath: path.join(repositoryRoot, runtimePath),
    runtimeFigureId: `mclone:${name}`,
    runtimePath,
  };
}
