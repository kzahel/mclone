import { Registry } from "../../../../core/registry";
import { ResourceLocation } from "../../../../core/resource-location";
import type { Block } from "../../../../world/level/block/block";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import { SimpleStateProvider } from "../stateproviders/simple-state-provider";
import type { BlockStateProvider } from "../stateproviders/block-state-provider";
import type { FeatureSize } from "../featuresize/feature-size";
import type { FoliagePlacer } from "../foliageplacers/foliage-placer";
import type { TreeDecorator } from "../treedecorators/tree-decorator";
import type { TrunkPlacer } from "../trunkplacers/trunk-placer";
import type { FeatureConfiguration } from "./feature-configuration";

const DIRT_LOCATION = new ResourceLocation("minecraft:dirt");

function getRequiredState(location: ResourceLocation): BlockState {
  const block = Registry.BLOCK.get(location) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class TreeConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly trunkProvider: BlockStateProvider,
    public readonly trunkPlacer: TrunkPlacer,
    public readonly foliageProvider: BlockStateProvider,
    public readonly saplingProvider: BlockStateProvider,
    public readonly foliagePlacer: FoliagePlacer,
    public readonly dirtProvider: BlockStateProvider,
    public readonly minimumSize: FeatureSize,
    public readonly decorators: readonly TreeDecorator[],
    public readonly ignoreVines: boolean,
    public readonly forceDirt: boolean,
  ) {}

  public withDecorators(decorators: readonly TreeDecorator[]): TreeConfiguration {
    return new TreeConfiguration(
      this.trunkProvider,
      this.trunkPlacer,
      this.foliageProvider,
      this.saplingProvider,
      this.foliagePlacer,
      this.dirtProvider,
      this.minimumSize,
      decorators,
      this.ignoreVines,
      this.forceDirt,
    );
  }
}

export namespace TreeConfiguration {
  export class TreeConfigurationBuilder {
    public readonly trunkProvider: BlockStateProvider;
    public readonly foliageProvider: BlockStateProvider;
    public readonly saplingProvider: BlockStateProvider;

    private dirtProvider: BlockStateProvider;
    private readonly foliagePlacer: FoliagePlacer;
    private readonly trunkPlacer: TrunkPlacer;
    private readonly minimumSize: FeatureSize;
    private decoratorsValue: readonly TreeDecorator[] = [];
    private ignoreVinesValue = false;
    private forceDirtValue = false;

    public constructor(
      trunkProvider: BlockStateProvider,
      trunkPlacer: TrunkPlacer,
      foliageProvider: BlockStateProvider,
      saplingProvider: BlockStateProvider,
      foliagePlacer: FoliagePlacer,
      minimumSize: FeatureSize,
    ) {
      this.trunkProvider = trunkProvider;
      this.trunkPlacer = trunkPlacer;
      this.foliageProvider = foliageProvider;
      this.saplingProvider = saplingProvider;
      this.dirtProvider = new SimpleStateProvider(getRequiredState(DIRT_LOCATION));
      this.foliagePlacer = foliagePlacer;
      this.minimumSize = minimumSize;
    }

    public dirt(dirtProvider: BlockStateProvider): TreeConfigurationBuilder {
      this.dirtProvider = dirtProvider;
      return this;
    }

    public decorators(decorators: readonly TreeDecorator[]): TreeConfigurationBuilder {
      this.decoratorsValue = decorators;
      return this;
    }

    public ignoreVines(): TreeConfigurationBuilder {
      this.ignoreVinesValue = true;
      return this;
    }

    public forceDirt(): TreeConfigurationBuilder {
      this.forceDirtValue = true;
      return this;
    }

    public build(): TreeConfiguration {
      return new TreeConfiguration(
        this.trunkProvider,
        this.trunkPlacer,
        this.foliageProvider,
        this.saplingProvider,
        this.foliagePlacer,
        this.dirtProvider,
        this.minimumSize,
        this.decoratorsValue,
        this.ignoreVinesValue,
        this.forceDirtValue,
      );
    }
  }
}
