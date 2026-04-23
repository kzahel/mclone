import { ResourceLocation } from "../../../core/resource-location";
import { Registry } from "../../../core/registry";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { ConstantInt } from "../../../util/valueproviders/constant-int";
import { BlobFoliagePlacer } from "./foliageplacers/blob-foliage-placer";
import { TreeConfiguration } from "./configurations/tree-configuration";
import { TwoLayersFeatureSize } from "./featuresize/two-layers-feature-size";
import { Features } from "./features";
import { SimpleStateProvider } from "./stateproviders/simple-state-provider";
import { StraightTrunkPlacer } from "./trunkplacers/straight-trunk-placer";

const OAK_LOG_LOCATION = new ResourceLocation("minecraft:oak_log");
const OAK_LEAVES_LOCATION = new ResourceLocation("minecraft:oak_leaves");
const OAK_SAPLING_LOCATION = new ResourceLocation("minecraft:oak_sapling");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class TreeFeatures {
  public static get OAK() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(OAK_LOG_LOCATION)),
        new StraightTrunkPlacer(4, 2, 0),
        new SimpleStateProvider(getRequiredState(OAK_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(OAK_SAPLING_LOCATION)),
        new BlobFoliagePlacer(ConstantInt.of(2), ConstantInt.of(0), 3),
        new TwoLayersFeatureSize(1, 0, 1),
      )
        .ignoreVines()
        .build(),
    );
  }
}
