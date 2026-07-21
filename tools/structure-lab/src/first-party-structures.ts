import path from "node:path";
import { repositoryRoot, structureLabRoot } from "./paths";

export interface FirstPartyStructure {
  name: string;
  outputPath: string;
  runtimeStatus: "lab-only" | "parity-canary" | "promoted";
  runtimeStructureId: string;
  runtimePath: string;
  sourcePath: string;
}

export const FIRST_PARTY_STRUCTURES: readonly FirstPartyStructure[] = [
  firstPartyStructure("cottage_snug_stoop", "farmstead-cottage-snug-stoop-v1"),
  firstPartyStructure("cottage_snug_canopy", "farmstead-cottage-snug-canopy-porch-v1"),
  firstPartyStructure("cottage_standard_stoop", "farmstead-cottage-standard-stoop-v1"),
  firstPartyStructure("cottage_standard", "farmstead-cottage-a-v2"),
  firstPartyStructure("cottage_deep_stoop", "farmstead-cottage-deep-stoop-v1"),
  firstPartyStructure("cottage_deep_canopy", "farmstead-cottage-deep-canopy-porch-v1"),
  firstPartyStructure("barn_core_short", "farmstead-barn-core-short-v1"),
  firstPartyStructure("barn_core_standard", "farmstead-barn-core-a-v2"),
  firstPartyStructure("barn_core_long", "farmstead-barn-core-long-v1"),
  firstPartyStructure("barn_lean_to_short", "farmstead-barn-lean-to-short-v1"),
  firstPartyStructure("barn_lean_to_standard", "farmstead-barn-lean-to-a-v2"),
  firstPartyStructure("barn_lean_to_long", "farmstead-barn-lean-to-long-v1"),
  firstPartyStructure(
    "coop_rosehip",
    "farmstead-rosehip-chicken-coop-v1",
    "lab-only",
  ),
];

function firstPartyStructure(
  name: string,
  id: string,
  runtimeStatus: FirstPartyStructure["runtimeStatus"] = "promoted",
): FirstPartyStructure {
  const runtimePath = `assets/mclone/structures/${id}.structure.json`;
  return {
    name,
    sourcePath: path.join(structureLabRoot, "examples", name, "structure.ts"),
    outputPath: path.join(repositoryRoot, runtimePath),
    runtimeStatus,
    runtimeStructureId: id,
    runtimePath,
  };
}
