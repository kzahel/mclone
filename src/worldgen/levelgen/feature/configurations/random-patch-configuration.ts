import type { Block } from "../../../../world/level/block/block";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { BlockPlacer } from "../blockplacers/block-placer";
import type { BlockStateProvider } from "../stateproviders/block-state-provider";
import type { FeatureConfiguration } from "./feature-configuration";

export class RandomPatchConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly stateProvider: BlockStateProvider,
    public readonly blockPlacer: BlockPlacer,
    public readonly whitelist: ReadonlySet<Block> = new Set(),
    public readonly blacklist: ReadonlySet<BlockState> = new Set(),
    public readonly tries = 128,
    public readonly xspread = 7,
    public readonly yspread = 3,
    public readonly zspread = 7,
    public readonly canReplace = false,
    public readonly project = true,
    public readonly needWater = false,
  ) {}

  public static grassConfigurationBuilder(stateProvider: BlockStateProvider, blockPlacer: BlockPlacer): RandomPatchConfiguration.GrassConfigurationBuilder {
    return new RandomPatchConfiguration.GrassConfigurationBuilder(stateProvider, blockPlacer);
  }
}

export namespace RandomPatchConfiguration {
  export class GrassConfigurationBuilder {
    private whitelist = new Set<Block>();
    private blacklist = new Set<BlockState>();
    private tries = 64;
    private xspread = 7;
    private yspread = 3;
    private zspread = 7;
    private canReplace = false;
    private project = true;
    private needWater = false;

    public constructor(
      private readonly stateProvider: BlockStateProvider,
      private readonly blockPlacer: BlockPlacer,
    ) {}

    public whitelistSet(value: ReadonlySet<Block>): GrassConfigurationBuilder {
      this.whitelist = new Set(value);
      return this;
    }

    public blacklistSet(value: ReadonlySet<BlockState>): GrassConfigurationBuilder {
      this.blacklist = new Set(value);
      return this;
    }

    public triesCount(value: number): GrassConfigurationBuilder {
      this.tries = value;
      return this;
    }

    public xspreadCount(value: number): GrassConfigurationBuilder {
      this.xspread = value;
      return this;
    }

    public yspreadCount(value: number): GrassConfigurationBuilder {
      this.yspread = value;
      return this;
    }

    public zspreadCount(value: number): GrassConfigurationBuilder {
      this.zspread = value;
      return this;
    }

    public canReplaceBlocks(): GrassConfigurationBuilder {
      this.canReplace = true;
      return this;
    }

    public noProjection(): GrassConfigurationBuilder {
      this.project = false;
      return this;
    }

    public needWaterAdjacency(): GrassConfigurationBuilder {
      this.needWater = true;
      return this;
    }

    public build(): RandomPatchConfiguration {
      return new RandomPatchConfiguration(
        this.stateProvider,
        this.blockPlacer,
        this.whitelist,
        this.blacklist,
        this.tries,
        this.xspread,
        this.yspread,
        this.zspread,
        this.canReplace,
        this.project,
        this.needWater,
      );
    }
  }
}
