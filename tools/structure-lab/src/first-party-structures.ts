import path from "node:path";
import { repositoryRoot, structureLabRoot } from "./paths";

export interface FirstPartyStructure {
  name: string;
  outputPath: string;
  runtimeStructureId: string;
  runtimePath: string;
  sourcePath: string;
}

export const FIRST_PARTY_STRUCTURES: readonly FirstPartyStructure[] = [
  firstPartyStructure("cottage_standard", "farmstead-cottage-a-v2"),
];

function firstPartyStructure(name: string, id: string): FirstPartyStructure {
  const runtimePath = `assets/mclone/structures/${id}.structure.json`;
  return {
    name,
    sourcePath: path.join(structureLabRoot, "examples", name, "structure.ts"),
    outputPath: path.join(repositoryRoot, runtimePath),
    runtimeStructureId: id,
    runtimePath,
  };
}
