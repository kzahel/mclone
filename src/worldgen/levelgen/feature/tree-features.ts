import { ResourceLocation } from "../../../core/resource-location";
import { Registry } from "../../../core/registry";
import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import { ConstantInt } from "../../../util/valueproviders/constant-int";
import { UniformInt } from "../../../util/valueproviders/uniform-int";
import { AcaciaFoliagePlacer } from "./foliageplacers/acacia-foliage-placer";
import { BlobFoliagePlacer } from "./foliageplacers/blob-foliage-placer";
import { BushFoliagePlacer } from "./foliageplacers/bush-foliage-placer";
import { DarkOakFoliagePlacer } from "./foliageplacers/dark-oak-foliage-placer";
import { FancyFoliagePlacer } from "./foliageplacers/fancy-foliage-placer";
import { MegaJungleFoliagePlacer } from "./foliageplacers/mega-jungle-foliage-placer";
import { PineFoliagePlacer } from "./foliageplacers/pine-foliage-placer";
import { SpruceFoliagePlacer } from "./foliageplacers/spruce-foliage-placer";
import { TreeConfiguration } from "./configurations/tree-configuration";
import { ThreeLayersFeatureSize } from "./featuresize/three-layers-feature-size";
import { TwoLayersFeatureSize } from "./featuresize/two-layers-feature-size";
import { Features } from "./features";
import { SimpleStateProvider } from "./stateproviders/simple-state-provider";
import { CocoaDecorator } from "./treedecorators/cocoa-decorator";
import { LeaveVineDecorator } from "./treedecorators/leave-vine-decorator";
import { TrunkVineDecorator } from "./treedecorators/trunk-vine-decorator";
import { DarkOakTrunkPlacer } from "./trunkplacers/dark-oak-trunk-placer";
import { FancyTrunkPlacer } from "./trunkplacers/fancy-trunk-placer";
import { ForkingTrunkPlacer } from "./trunkplacers/forking-trunk-placer";
import { MegaJungleTrunkPlacer } from "./trunkplacers/mega-jungle-trunk-placer";
import { StraightTrunkPlacer } from "./trunkplacers/straight-trunk-placer";

const OAK_LOG_LOCATION = new ResourceLocation("minecraft:oak_log");
const OAK_LEAVES_LOCATION = new ResourceLocation("minecraft:oak_leaves");
const OAK_SAPLING_LOCATION = new ResourceLocation("minecraft:oak_sapling");
const ACACIA_LOG_LOCATION = new ResourceLocation("minecraft:acacia_log");
const ACACIA_LEAVES_LOCATION = new ResourceLocation("minecraft:acacia_leaves");
const ACACIA_SAPLING_LOCATION = new ResourceLocation("minecraft:acacia_sapling");
const SPRUCE_LOG_LOCATION = new ResourceLocation("minecraft:spruce_log");
const SPRUCE_LEAVES_LOCATION = new ResourceLocation("minecraft:spruce_leaves");
const SPRUCE_SAPLING_LOCATION = new ResourceLocation("minecraft:spruce_sapling");
const BIRCH_LOG_LOCATION = new ResourceLocation("minecraft:birch_log");
const BIRCH_LEAVES_LOCATION = new ResourceLocation("minecraft:birch_leaves");
const BIRCH_SAPLING_LOCATION = new ResourceLocation("minecraft:birch_sapling");
const DARK_OAK_LOG_LOCATION = new ResourceLocation("minecraft:dark_oak_log");
const DARK_OAK_LEAVES_LOCATION = new ResourceLocation("minecraft:dark_oak_leaves");
const DARK_OAK_SAPLING_LOCATION = new ResourceLocation("minecraft:dark_oak_sapling");
const JUNGLE_LOG_LOCATION = new ResourceLocation("minecraft:jungle_log");
const JUNGLE_LEAVES_LOCATION = new ResourceLocation("minecraft:jungle_leaves");
const JUNGLE_SAPLING_LOCATION = new ResourceLocation("minecraft:jungle_sapling");

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

  public static get ACACIA() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(ACACIA_LOG_LOCATION)),
        new ForkingTrunkPlacer(5, 2, 2),
        new SimpleStateProvider(getRequiredState(ACACIA_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(ACACIA_SAPLING_LOCATION)),
        new AcaciaFoliagePlacer(ConstantInt.of(2), ConstantInt.of(0)),
        new TwoLayersFeatureSize(1, 0, 2),
      )
        .ignoreVines()
        .build(),
    );
  }

  public static get JUNGLE_TREE() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(JUNGLE_LOG_LOCATION)),
        new StraightTrunkPlacer(4, 8, 0),
        new SimpleStateProvider(getRequiredState(JUNGLE_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(JUNGLE_SAPLING_LOCATION)),
        new BlobFoliagePlacer(ConstantInt.of(2), ConstantInt.of(0), 3),
        new TwoLayersFeatureSize(1, 0, 1),
      )
        .decorators([new CocoaDecorator(0.2), TrunkVineDecorator.INSTANCE, LeaveVineDecorator.INSTANCE])
        .ignoreVines()
        .build(),
    );
  }

  public static get SPRUCE() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(SPRUCE_LOG_LOCATION)),
        new StraightTrunkPlacer(5, 2, 1),
        new SimpleStateProvider(getRequiredState(SPRUCE_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(SPRUCE_SAPLING_LOCATION)),
        new SpruceFoliagePlacer(UniformInt.of(2, 3), UniformInt.of(0, 2), UniformInt.of(1, 2)),
        new TwoLayersFeatureSize(2, 0, 2),
      )
        .ignoreVines()
        .build(),
    );
  }

  public static get PINE() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(SPRUCE_LOG_LOCATION)),
        new StraightTrunkPlacer(6, 4, 0),
        new SimpleStateProvider(getRequiredState(SPRUCE_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(SPRUCE_SAPLING_LOCATION)),
        new PineFoliagePlacer(ConstantInt.of(1), ConstantInt.of(1), UniformInt.of(3, 4)),
        new TwoLayersFeatureSize(2, 0, 2),
      )
        .ignoreVines()
        .build(),
    );
  }

  public static get FANCY_OAK() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(OAK_LOG_LOCATION)),
        new FancyTrunkPlacer(3, 11, 0),
        new SimpleStateProvider(getRequiredState(OAK_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(OAK_SAPLING_LOCATION)),
        new FancyFoliagePlacer(ConstantInt.of(2), ConstantInt.of(4), 4),
        new TwoLayersFeatureSize(0, 0, 0, 4),
      )
        .ignoreVines()
        .build(),
    );
  }

  public static get JUNGLE_TREE_NO_VINE() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(JUNGLE_LOG_LOCATION)),
        new StraightTrunkPlacer(4, 8, 0),
        new SimpleStateProvider(getRequiredState(JUNGLE_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(JUNGLE_SAPLING_LOCATION)),
        new BlobFoliagePlacer(ConstantInt.of(2), ConstantInt.of(0), 3),
        new TwoLayersFeatureSize(1, 0, 1),
      )
        .ignoreVines()
        .build(),
    );
  }

  public static get MEGA_JUNGLE_TREE() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(JUNGLE_LOG_LOCATION)),
        new MegaJungleTrunkPlacer(10, 2, 19),
        new SimpleStateProvider(getRequiredState(JUNGLE_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(JUNGLE_SAPLING_LOCATION)),
        new MegaJungleFoliagePlacer(ConstantInt.of(2), ConstantInt.of(0), 2),
        new TwoLayersFeatureSize(1, 1, 2),
      )
        .decorators([TrunkVineDecorator.INSTANCE, LeaveVineDecorator.INSTANCE])
        .build(),
    );
  }

  public static get BIRCH() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(BIRCH_LOG_LOCATION)),
        new StraightTrunkPlacer(5, 2, 0),
        new SimpleStateProvider(getRequiredState(BIRCH_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(BIRCH_SAPLING_LOCATION)),
        new BlobFoliagePlacer(ConstantInt.of(2), ConstantInt.of(0), 3),
        new TwoLayersFeatureSize(1, 0, 1),
      )
        .ignoreVines()
        .build(),
    );
  }

  public static get DARK_OAK() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(DARK_OAK_LOG_LOCATION)),
        new DarkOakTrunkPlacer(6, 2, 1),
        new SimpleStateProvider(getRequiredState(DARK_OAK_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(DARK_OAK_SAPLING_LOCATION)),
        new DarkOakFoliagePlacer(ConstantInt.of(0), ConstantInt.of(0)),
        new ThreeLayersFeatureSize(1, 1, 0, 1, 2),
      )
        .ignoreVines()
        .build(),
    );
  }

  public static get SUPER_BIRCH() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(BIRCH_LOG_LOCATION)),
        new StraightTrunkPlacer(5, 2, 6),
        new SimpleStateProvider(getRequiredState(BIRCH_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(BIRCH_SAPLING_LOCATION)),
        new BlobFoliagePlacer(ConstantInt.of(2), ConstantInt.of(0), 3),
        new TwoLayersFeatureSize(1, 0, 1),
      )
        .ignoreVines()
        .build(),
    );
  }

  public static get SWAMP_OAK() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(OAK_LOG_LOCATION)),
        new StraightTrunkPlacer(5, 3, 0),
        new SimpleStateProvider(getRequiredState(OAK_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(OAK_SAPLING_LOCATION)),
        new BlobFoliagePlacer(ConstantInt.of(3), ConstantInt.of(0), 3),
        new TwoLayersFeatureSize(1, 0, 1),
      ).build(),
    );
  }

  public static get JUNGLE_BUSH() {
    return Features.TREE.configured(
      new TreeConfiguration.TreeConfigurationBuilder(
        new SimpleStateProvider(getRequiredState(JUNGLE_LOG_LOCATION)),
        new StraightTrunkPlacer(1, 0, 0),
        new SimpleStateProvider(getRequiredState(OAK_LEAVES_LOCATION)),
        new SimpleStateProvider(getRequiredState(OAK_SAPLING_LOCATION)),
        new BushFoliagePlacer(ConstantInt.of(2), ConstantInt.of(1), 2),
        new TwoLayersFeatureSize(0, 0, 0),
      ).build(),
    );
  }
}
