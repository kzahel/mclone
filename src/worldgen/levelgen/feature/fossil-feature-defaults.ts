import { ResourceLocation } from "../../../core/resource-location";
import { BlockTags } from "../../../tags/block-tags";
import { BlockRotProcessor } from "../../../world/level/levelgen/structure/templatesystem/block-rot-processor";
import { ProtectedBlockProcessor } from "../../../world/level/levelgen/structure/templatesystem/protected-block-processor";
import { StructureProcessorList } from "../../../world/level/levelgen/structure/templatesystem/structure-processor-list";
import { FossilFeatureConfiguration } from "./configurations/fossil-feature-configuration";

const FOSSIL_STRUCTURES = [
  "minecraft:fossil/spine_1",
  "minecraft:fossil/spine_2",
  "minecraft:fossil/spine_3",
  "minecraft:fossil/spine_4",
  "minecraft:fossil/skull_1",
  "minecraft:fossil/skull_2",
  "minecraft:fossil/skull_3",
  "minecraft:fossil/skull_4",
].map((location) => new ResourceLocation(location));

const FOSSIL_COAL_STRUCTURES = [
  "minecraft:fossil/spine_1_coal",
  "minecraft:fossil/spine_2_coal",
  "minecraft:fossil/spine_3_coal",
  "minecraft:fossil/spine_4_coal",
  "minecraft:fossil/skull_1_coal",
  "minecraft:fossil/skull_2_coal",
  "minecraft:fossil/skull_3_coal",
  "minecraft:fossil/skull_4_coal",
].map((location) => new ResourceLocation(location));

const FOSSIL_ROT = new StructureProcessorList([
  new BlockRotProcessor(0.9),
  new ProtectedBlockProcessor(BlockTags.FEATURES_CANNOT_REPLACE),
]);

const FOSSIL_COAL = new StructureProcessorList([
  new BlockRotProcessor(0.1),
  new ProtectedBlockProcessor(BlockTags.FEATURES_CANNOT_REPLACE),
]);

export const OVERWORLD_FOSSIL_CONFIGURATION = new FossilFeatureConfiguration(
  FOSSIL_STRUCTURES,
  FOSSIL_COAL_STRUCTURES,
  () => FOSSIL_ROT,
  () => FOSSIL_COAL,
  4,
);
