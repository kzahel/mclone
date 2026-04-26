import { LazyAreaContext, type Area, type AreaFactory, type BigContext, type RandomContext } from "./area";

const WARM_ID = 1;
const MEDIUM_ID = 2;
const COLD_ID = 3;
const ICE_ID = 4;
const SPECIAL_MASK = 3840;
const SPECIAL_SHIFT = 8;

type UnaryAreaTransformer = {
  run(context: LazyAreaContext, areaFactory: AreaFactory): AreaFactory;
};

type BinaryAreaTransformer = {
  run(context: LazyAreaContext, firstAreaFactory: AreaFactory, secondAreaFactory: AreaFactory): AreaFactory;
};

type NullaryAreaTransformer = {
  run(context: LazyAreaContext): AreaFactory;
};

function runAreaTransformer0(
  context: LazyAreaContext,
  applyPixel: (context: RandomContext, x: number, z: number) => number,
): AreaFactory {
  return () =>
    context.createResult((x, z) => {
      context.initRandom(x, z);
      return applyPixel(context, x, z);
    });
}

function runAreaTransformer1(
  context: LazyAreaContext,
  areaFactory: AreaFactory,
  applyPixel: (context: BigContext, area: Area, x: number, z: number) => number,
): AreaFactory {
  return () => {
    const area = areaFactory();
    return context.createResult(
      (x, z) => {
        context.initRandom(x, z);
        return applyPixel(context, area, x, z);
      },
      area,
    );
  };
}

function runAreaTransformer2(
  context: LazyAreaContext,
  firstAreaFactory: AreaFactory,
  secondAreaFactory: AreaFactory,
  applyPixel: (context: RandomContext, firstArea: Area, secondArea: Area, x: number, z: number) => number,
): AreaFactory {
  return () => {
    const firstArea = firstAreaFactory();
    const secondArea = secondAreaFactory();
    return context.createResult(
      (x, z) => {
        context.initRandom(x, z);
        return applyPixel(context, firstArea, secondArea, x, z);
      },
      firstArea,
      secondArea,
    );
  };
}

function runC0Transformer(
  context: LazyAreaContext,
  areaFactory: AreaFactory,
  apply: (context: RandomContext, value: number) => number,
): AreaFactory {
  return runAreaTransformer1(context, areaFactory, (bigContext, area, x, z) => apply(bigContext, area.get(x, z)));
}

function runC1Transformer(
  context: LazyAreaContext,
  areaFactory: AreaFactory,
  apply: (context: RandomContext, value: number) => number,
): AreaFactory {
  return runAreaTransformer1(context, areaFactory, (bigContext, area, x, z) => apply(bigContext, area.get(x - 1 + 1, z - 1 + 1)));
}

function runCastleTransformer(
  context: LazyAreaContext,
  areaFactory: AreaFactory,
  apply: (context: RandomContext, north: number, east: number, south: number, west: number, center: number) => number,
): AreaFactory {
  return runAreaTransformer1(context, areaFactory, (bigContext, area, x, z) =>
    apply(
      bigContext,
      area.get(x - 1 + 1, z - 1 + 0),
      area.get(x - 1 + 2, z - 1 + 1),
      area.get(x - 1 + 1, z - 1 + 2),
      area.get(x - 1 + 0, z - 1 + 1),
      area.get(x - 1 + 1, z - 1 + 1),
    ),
  );
}

function runBishopTransformer(
  context: LazyAreaContext,
  areaFactory: AreaFactory,
  apply: (context: RandomContext, southWest: number, southEast: number, northEast: number, northWest: number, center: number) => number,
): AreaFactory {
  return runAreaTransformer1(context, areaFactory, (bigContext, area, x, z) =>
    apply(
      bigContext,
      area.get(x - 1 + 0, z - 1 + 2),
      area.get(x - 1 + 2, z - 1 + 2),
      area.get(x - 1 + 2, z - 1 + 0),
      area.get(x - 1 + 0, z - 1 + 0),
      area.get(x - 1 + 1, z - 1 + 1),
    ),
  );
}

function registerCategory(categories: Map<number, number>, category: number, biomeId: number): void {
  categories.set(biomeId, category);
}

const CATEGORIES = (() => {
  const categories = new Map<number, number>();
  registerCategory(categories, 9, 16);
  registerCategory(categories, 9, 26);
  registerCategory(categories, 12, 2);
  registerCategory(categories, 12, 17);
  registerCategory(categories, 12, 130);
  registerCategory(categories, 2, 131);
  registerCategory(categories, 2, 162);
  registerCategory(categories, 2, 20);
  registerCategory(categories, 2, 3);
  registerCategory(categories, 2, 34);
  registerCategory(categories, 10, 27);
  registerCategory(categories, 10, 28);
  registerCategory(categories, 10, 29);
  registerCategory(categories, 10, 157);
  registerCategory(categories, 10, 132);
  registerCategory(categories, 10, 4);
  registerCategory(categories, 10, 155);
  registerCategory(categories, 10, 156);
  registerCategory(categories, 10, 18);
  registerCategory(categories, 8, 140);
  registerCategory(categories, 8, 13);
  registerCategory(categories, 8, 12);
  registerCategory(categories, 3, 168);
  registerCategory(categories, 3, 169);
  registerCategory(categories, 3, 21);
  registerCategory(categories, 3, 23);
  registerCategory(categories, 3, 22);
  registerCategory(categories, 3, 149);
  registerCategory(categories, 3, 151);
  registerCategory(categories, 4, 37);
  registerCategory(categories, 4, 165);
  registerCategory(categories, 4, 167);
  registerCategory(categories, 4, 166);
  registerCategory(categories, 5, 39);
  registerCategory(categories, 5, 38);
  registerCategory(categories, 15, 14);
  registerCategory(categories, 15, 15);
  registerCategory(categories, 0, 25);
  registerCategory(categories, 11, 46);
  registerCategory(categories, 11, 49);
  registerCategory(categories, 11, 50);
  registerCategory(categories, 11, 48);
  registerCategory(categories, 11, 24);
  registerCategory(categories, 11, 47);
  registerCategory(categories, 11, 10);
  registerCategory(categories, 11, 45);
  registerCategory(categories, 11, 0);
  registerCategory(categories, 11, 44);
  registerCategory(categories, 6, 1);
  registerCategory(categories, 6, 129);
  registerCategory(categories, 13, 11);
  registerCategory(categories, 13, 7);
  registerCategory(categories, 7, 35);
  registerCategory(categories, 7, 36);
  registerCategory(categories, 7, 163);
  registerCategory(categories, 7, 164);
  registerCategory(categories, 14, 6);
  registerCategory(categories, 14, 134);
  registerCategory(categories, 1, 160);
  registerCategory(categories, 1, 161);
  registerCategory(categories, 1, 32);
  registerCategory(categories, 1, 33);
  registerCategory(categories, 1, 30);
  registerCategory(categories, 1, 31);
  registerCategory(categories, 1, 158);
  registerCategory(categories, 1, 5);
  registerCategory(categories, 1, 19);
  registerCategory(categories, 1, 133);
  return categories;
})();

function isSame(left: number, right: number): boolean {
  return left === right || CATEGORIES.get(left) === CATEGORIES.get(right);
}

function isOcean(biome: number): boolean {
  return biome === 44 || biome === 45 || biome === 0 || biome === 46 || biome === 10 || biome === 47 || biome === 48 || biome === 24 || biome === 49 || biome === 50;
}

function isShallowOcean(biome: number): boolean {
  return biome === 44 || biome === 45 || biome === 0 || biome === 46 || biome === 10;
}

const IslandLayer: NullaryAreaTransformer = {
  run(context) {
    return runAreaTransformer0(context, (randomContext, x, z) => {
      if (x === 0 && z === 0) {
        return 1;
      }

      return randomContext.nextRandom(10) === 0 ? 1 : 0;
    });
  },
};

const AddIslandLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runBishopTransformer(context, areaFactory, (randomContext, southWest, southEast, northEast, northWest, center) => {
      if (
        !isShallowOcean(center) ||
        (isShallowOcean(northWest) && isShallowOcean(northEast) && isShallowOcean(southWest) && isShallowOcean(southEast))
      ) {
        if (
          !isShallowOcean(center) &&
          (isShallowOcean(northWest) || isShallowOcean(southWest) || isShallowOcean(northEast) || isShallowOcean(southEast)) &&
          randomContext.nextRandom(5) === 0
        ) {
          if (isShallowOcean(northWest)) {
            return center === ICE_ID ? ICE_ID : northWest;
          }

          if (isShallowOcean(southWest)) {
            return center === ICE_ID ? ICE_ID : southWest;
          }

          if (isShallowOcean(northEast)) {
            return center === ICE_ID ? ICE_ID : northEast;
          }

          if (isShallowOcean(southEast)) {
            return center === ICE_ID ? ICE_ID : southEast;
          }
        }

        return center;
      }

      let candidateCount = 1;
      let candidate = 1;
      if (!isShallowOcean(northWest) && randomContext.nextRandom(candidateCount++) === 0) {
        candidate = northWest;
      }

      if (!isShallowOcean(northEast) && randomContext.nextRandom(candidateCount++) === 0) {
        candidate = northEast;
      }

      if (!isShallowOcean(southWest) && randomContext.nextRandom(candidateCount++) === 0) {
        candidate = southWest;
      }

      if (!isShallowOcean(southEast) && randomContext.nextRandom(candidateCount++) === 0) {
        candidate = southEast;
      }

      if (randomContext.nextRandom(3) === 0) {
        return candidate;
      }

      return candidate === ICE_ID ? ICE_ID : center;
    });
  },
};

const RemoveTooMuchOceanLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runCastleTransformer(context, areaFactory, (randomContext, north, east, south, west, center) =>
      isShallowOcean(center) && isShallowOcean(north) && isShallowOcean(east) && isShallowOcean(south) && isShallowOcean(west) && randomContext.nextRandom(2) === 0
        ? 1
        : center,
    );
  },
};

const AddSnowLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runC1Transformer(context, areaFactory, (randomContext, value) => {
      if (isShallowOcean(value)) {
        return value;
      }

      const roll = randomContext.nextRandom(6);
      if (roll === 0) {
        return ICE_ID;
      }

      return roll === 1 ? COLD_ID : WARM_ID;
    });
  },
};

const AddEdgeLayerCoolWarm: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runCastleTransformer(context, areaFactory, (_randomContext, north, east, south, west, center) =>
      center !== WARM_ID ||
      ((north !== COLD_ID && east !== COLD_ID && south !== COLD_ID && west !== COLD_ID) &&
        (north !== ICE_ID && east !== ICE_ID && south !== ICE_ID && west !== ICE_ID))
        ? center
        : MEDIUM_ID,
    );
  },
};

const AddEdgeLayerHeatIce: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runCastleTransformer(context, areaFactory, (_randomContext, north, east, south, west, center) =>
      center !== ICE_ID ||
      ((north !== WARM_ID && east !== WARM_ID && south !== WARM_ID && west !== WARM_ID) &&
        (north !== MEDIUM_ID && east !== MEDIUM_ID && south !== MEDIUM_ID && west !== MEDIUM_ID))
        ? center
        : COLD_ID,
    );
  },
};

const AddEdgeLayerIntroduceSpecial: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runC0Transformer(context, areaFactory, (randomContext, value) => {
      if (!isShallowOcean(value) && randomContext.nextRandom(13) === 0) {
        value |= ((1 + randomContext.nextRandom(15)) << SPECIAL_SHIFT) & SPECIAL_MASK;
      }

      return value;
    });
  },
};

const AddMushroomIslandLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runBishopTransformer(
      context,
      areaFactory,
      (randomContext, southWest, southEast, northEast, northWest, center) =>
        isShallowOcean(center) &&
        isShallowOcean(northWest) &&
        isShallowOcean(southWest) &&
        isShallowOcean(northEast) &&
        isShallowOcean(southEast) &&
        randomContext.nextRandom(100) === 0
          ? 14
          : center,
    );
  },
};

const AddDeepOceanLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runCastleTransformer(context, areaFactory, (_randomContext, north, east, south, west, center) => {
      if (!isShallowOcean(center)) {
        return center;
      }

      let shallowOceanNeighbors = 0;
      if (isShallowOcean(north)) {
        shallowOceanNeighbors++;
      }

      if (isShallowOcean(west)) {
        shallowOceanNeighbors++;
      }

      if (isShallowOcean(east)) {
        shallowOceanNeighbors++;
      }

      if (isShallowOcean(south)) {
        shallowOceanNeighbors++;
      }

      if (shallowOceanNeighbors <= 3) {
        return center;
      }

      switch (center) {
        case 44:
          return 47;
        case 45:
          return 48;
        case 0:
          return 24;
        case 46:
          return 49;
        case 10:
          return 50;
        default:
          return 24;
      }
    });
  },
};

const RiverInitLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runC0Transformer(context, areaFactory, (randomContext, value) => (isShallowOcean(value) ? value : randomContext.nextRandom(299999) + 2));
  },
};

class BiomeInitLayer implements UnaryAreaTransformer {
  private static readonly LEGACY_WARM_BIOMES = [2, 4, 3, 6, 1, 5] as const;
  private static readonly WARM_BIOMES = [2, 2, 2, 35, 35, 1] as const;
  private static readonly MEDIUM_BIOMES = [4, 29, 3, 1, 27, 6] as const;
  private static readonly COLD_BIOMES = [4, 3, 5, 1] as const;
  private static readonly ICE_BIOMES = [12, 12, 12, 30] as const;

  private readonly warmBiomes: readonly number[];

  public constructor(legacyBiomeInitLayer: boolean) {
    this.warmBiomes = legacyBiomeInitLayer ? BiomeInitLayer.LEGACY_WARM_BIOMES : BiomeInitLayer.WARM_BIOMES;
  }

  public run(context: LazyAreaContext, areaFactory: AreaFactory): AreaFactory {
    return runC0Transformer(context, areaFactory, (randomContext, value) => {
      const specialBits = (value & SPECIAL_MASK) >> SPECIAL_SHIFT;
      value &= ~SPECIAL_MASK;
      if (isOcean(value) || value === 14) {
        return value;
      }

      switch (value) {
        case WARM_ID:
          if (specialBits > 0) {
            return randomContext.nextRandom(3) === 0 ? 39 : 38;
          }

          return this.warmBiomes[randomContext.nextRandom(this.warmBiomes.length)]!;
        case MEDIUM_ID:
          if (specialBits > 0) {
            return 21;
          }

          return BiomeInitLayer.MEDIUM_BIOMES[randomContext.nextRandom(BiomeInitLayer.MEDIUM_BIOMES.length)]!;
        case COLD_ID:
          if (specialBits > 0) {
            return 32;
          }

          return BiomeInitLayer.COLD_BIOMES[randomContext.nextRandom(BiomeInitLayer.COLD_BIOMES.length)]!;
        case ICE_ID:
          return BiomeInitLayer.ICE_BIOMES[randomContext.nextRandom(BiomeInitLayer.ICE_BIOMES.length)]!;
        default:
          return 14;
      }
    });
  }
}

const RareBiomeLargeLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runC1Transformer(context, areaFactory, (randomContext, value) => (randomContext.nextRandom(10) === 0 && value === 21 ? 168 : value));
  },
};

class ZoomLayerImpl implements UnaryAreaTransformer {
  public constructor(private readonly fuzzy: boolean) {}

  public run(context: LazyAreaContext, areaFactory: AreaFactory): AreaFactory {
    return runAreaTransformer1(context, areaFactory, (bigContext, area, x, z) => {
      const parentX = x >> 1;
      const parentZ = z >> 1;
      const center = area.get(parentX, parentZ);
      bigContext.initRandom((x >> 1) << 1, (z >> 1) << 1);
      const oddX = x & 1;
      const oddZ = z & 1;
      if (oddX === 0 && oddZ === 0) {
        return center;
      }

      const south = area.get(parentX, (z + 1) >> 1);
      const centerSouth = bigContext.random(center, south);
      if (oddX === 0 && oddZ === 1) {
        return centerSouth;
      }

      const east = area.get((x + 1) >> 1, parentZ);
      const centerEast = bigContext.random(center, east);
      if (oddX === 1 && oddZ === 0) {
        return centerEast;
      }

      const southEast = area.get((x + 1) >> 1, (z + 1) >> 1);
      return this.modeOrRandom(bigContext, center, east, south, southEast);
    });
  }

  private modeOrRandom(context: BigContext, first: number, second: number, third: number, fourth: number): number {
    if (this.fuzzy) {
      return context.random(first, second, third, fourth);
    }

    if (second === third && third === fourth) {
      return second;
    }
    if (first === second && first === third) {
      return first;
    }
    if (first === second && first === fourth) {
      return first;
    }
    if (first === third && first === fourth) {
      return first;
    }
    if (first === second && third !== fourth) {
      return first;
    }
    if (first === third && second !== fourth) {
      return first;
    }
    if (first === fourth && second !== third) {
      return first;
    }
    if (second === third && first !== fourth) {
      return second;
    }
    if (second === fourth && first !== third) {
      return second;
    }
    if (third === fourth && first !== second) {
      return third;
    }

    return context.random(first, second, third, fourth);
  }
}

const ZoomLayer = {
  NORMAL: new ZoomLayerImpl(false),
  FUZZY: new ZoomLayerImpl(true),
};

const BiomeEdgeLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runCastleTransformer(context, areaFactory, (_randomContext, north, east, south, west, center) => {
      const edge = { value: 0 };
      if (
        checkEdge(edge, center) ||
        checkEdgeStrict(edge, north, east, south, west, center, 38, 37) ||
        checkEdgeStrict(edge, north, east, south, west, center, 39, 37) ||
        checkEdgeStrict(edge, north, east, south, west, center, 32, 5)
      ) {
        return edge.value;
      }

      if (center === 2 && (north === 12 || east === 12 || south === 12 || west === 12)) {
        return 34;
      }

      if (center === 6) {
        if (
          north === 2 ||
          east === 2 ||
          south === 2 ||
          west === 2 ||
          north === 30 ||
          east === 30 ||
          south === 30 ||
          west === 30 ||
          north === 12 ||
          east === 12 ||
          south === 12 ||
          west === 12
        ) {
          return 1;
        }

        if (north === 21 || east === 21 || south === 21 || west === 21 || north === 168 || east === 168 || south === 168 || west === 168) {
          return 23;
        }
      }

      return center;
    });
  },
};

function checkEdge(result: { value: number }, edge: number): boolean {
  if (!isSame(edge, 3)) {
    return false;
  }

  result.value = edge;
  return true;
}

function checkEdgeStrict(
  result: { value: number },
  north: number,
  east: number,
  south: number,
  west: number,
  center: number,
  check: number,
  edge: number,
): boolean {
  if (center !== check) {
    return false;
  }

  result.value = isSame(north, check) && isSame(east, check) && isSame(south, check) && isSame(west, check) ? center : edge;
  return true;
}

const REGION_HILLS_MUTATIONS = new Map<number, number>([
  [1, 129],
  [2, 130],
  [3, 131],
  [4, 132],
  [5, 133],
  [6, 134],
  [12, 140],
  [21, 149],
  [23, 151],
  [27, 155],
  [28, 156],
  [29, 157],
  [30, 158],
  [32, 160],
  [33, 161],
  [34, 162],
  [35, 163],
  [36, 164],
  [37, 165],
  [38, 166],
  [39, 167],
]);

const RegionHillsLayer: BinaryAreaTransformer = {
  run(context, firstAreaFactory, secondAreaFactory) {
    return runAreaTransformer2(context, firstAreaFactory, secondAreaFactory, (randomContext, firstArea, secondArea, x, z) => {
      const center = firstArea.get(x, z);
      const riverValue = secondArea.get(x, z);
      const mutationRoll = (riverValue - 2) % 29;
      if (!isShallowOcean(center) && riverValue >= 2 && mutationRoll === 1) {
        return REGION_HILLS_MUTATIONS.get(center) ?? center;
      }

      if (randomContext.nextRandom(3) !== 0 && mutationRoll !== 0) {
        return center;
      }

      let mutated = center;
      if (center === 2) {
        mutated = 17;
      } else if (center === 4) {
        mutated = 18;
      } else if (center === 27) {
        mutated = 28;
      } else if (center === 29) {
        mutated = 1;
      } else if (center === 5) {
        mutated = 19;
      } else if (center === 32) {
        mutated = 33;
      } else if (center === 30) {
        mutated = 31;
      } else if (center === 1) {
        mutated = randomContext.nextRandom(3) === 0 ? 18 : 4;
      } else if (center === 12) {
        mutated = 13;
      } else if (center === 21) {
        mutated = 22;
      } else if (center === 168) {
        mutated = 169;
      } else if (center === 0) {
        mutated = 24;
      } else if (center === 45) {
        mutated = 48;
      } else if (center === 46) {
        mutated = 49;
      } else if (center === 10) {
        mutated = 50;
      } else if (center === 3) {
        mutated = 34;
      } else if (center === 35) {
        mutated = 36;
      } else if (isSame(center, 38)) {
        mutated = 37;
      } else if ((center === 24 || center === 48 || center === 49 || center === 50) && randomContext.nextRandom(3) === 0) {
        mutated = randomContext.nextRandom(2) === 0 ? 1 : 4;
      }

      if (mutationRoll === 0 && mutated !== center) {
        mutated = REGION_HILLS_MUTATIONS.get(mutated) ?? center;
      }

      if (mutated === center) {
        return center;
      }

      let matchingNeighbors = 0;
      if (isSame(firstArea.get(x, z - 1), center)) {
        matchingNeighbors++;
      }
      if (isSame(firstArea.get(x + 1, z), center)) {
        matchingNeighbors++;
      }
      if (isSame(firstArea.get(x - 1, z), center)) {
        matchingNeighbors++;
      }
      if (isSame(firstArea.get(x, z + 1), center)) {
        matchingNeighbors++;
      }

      return matchingNeighbors >= 3 ? mutated : center;
    });
  },
};

const RiverLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runCastleTransformer(context, areaFactory, (_randomContext, north, east, south, west, center) => {
      const filteredCenter = riverFilter(center);
      return filteredCenter === riverFilter(east) && filteredCenter === riverFilter(north) && filteredCenter === riverFilter(west) && filteredCenter === riverFilter(south)
        ? -1
        : 7;
    });
  },
};

function riverFilter(value: number): number {
  return value >= 2 ? 2 + (value & 1) : value;
}

const SmoothLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runCastleTransformer(context, areaFactory, (randomContext, north, east, south, west, center) => {
      const horizontalMatch = west === east;
      const verticalMatch = north === south;
      if (horizontalMatch === verticalMatch) {
        if (horizontalMatch) {
          return randomContext.nextRandom(2) === 0 ? east : north;
        }

        return center;
      }

      return horizontalMatch ? east : north;
    });
  },
};

const RareBiomeSpotLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runC1Transformer(context, areaFactory, (randomContext, value) => (randomContext.nextRandom(57) === 0 && value === 1 ? 129 : value));
  },
};

const SNOWY_BIOMES = new Set([26, 11, 12, 13, 140, 30, 31, 158, 10]);
const JUNGLE_BIOMES = new Set([168, 169, 21, 22, 23, 149, 151]);

const ShoreLayer: UnaryAreaTransformer = {
  run(context, areaFactory) {
    return runCastleTransformer(context, areaFactory, (_randomContext, north, east, south, west, center) => {
      if (center === 14) {
        return isShallowOcean(north) || isShallowOcean(east) || isShallowOcean(south) || isShallowOcean(west) ? 15 : center;
      }

      if (JUNGLE_BIOMES.has(center)) {
        if (!isJungleCompatible(north) || !isJungleCompatible(east) || !isJungleCompatible(south) || !isJungleCompatible(west)) {
          return 23;
        }

        if (isOcean(north) || isOcean(east) || isOcean(south) || isOcean(west)) {
          return 16;
        }

        return center;
      }

      if (center === 3 || center === 34 || center === 20) {
        return !isOcean(center) && (isOcean(north) || isOcean(east) || isOcean(south) || isOcean(west)) ? 25 : center;
      }

      if (SNOWY_BIOMES.has(center)) {
        return !isOcean(center) && (isOcean(north) || isOcean(east) || isOcean(south) || isOcean(west)) ? 26 : center;
      }

      if (center === 37 || center === 38) {
        if (!isOcean(north) && !isOcean(east) && !isOcean(south) && !isOcean(west) && (!isMesa(north) || !isMesa(east) || !isMesa(south) || !isMesa(west))) {
          return 2;
        }

        return center;
      }

      return !isOcean(center) && center !== 7 && center !== 6 && (isOcean(north) || isOcean(east) || isOcean(south) || isOcean(west)) ? 16 : center;
    });
  },
};

function isJungleCompatible(value: number): boolean {
  return JUNGLE_BIOMES.has(value) || value === 4 || value === 5 || isOcean(value);
}

function isMesa(value: number): boolean {
  return value === 37 || value === 38 || value === 39 || value === 165 || value === 166 || value === 167;
}

const OceanLayer: NullaryAreaTransformer = {
  run(context) {
    return runAreaTransformer0(context, (randomContext, x, z) => {
      const biomeNoise = randomContext.getBiomeNoise();
      const value = biomeNoise.noise(x / 8.0, z / 8.0, 0.0);
      if (value > 0.4) {
        return 44;
      }
      if (value > 0.2) {
        return 45;
      }
      if (value < -0.4) {
        return 10;
      }

      return value < -0.2 ? 46 : 0;
    });
  },
};

const RiverMixerLayer: BinaryAreaTransformer = {
  run(context, firstAreaFactory, secondAreaFactory) {
    return runAreaTransformer2(context, firstAreaFactory, secondAreaFactory, (_randomContext, firstArea, secondArea, x, z) => {
      const land = firstArea.get(x, z);
      const river = secondArea.get(x, z);
      if (isOcean(land)) {
        return land;
      }

      if (river !== 7) {
        return land;
      }

      if (land === 12) {
        return 11;
      }

      return land !== 14 && land !== 15 ? river & 0xff : 15;
    });
  },
};

export const OceanMixerLayer: BinaryAreaTransformer = {
  run(context, firstAreaFactory, secondAreaFactory) {
    return runAreaTransformer2(context, firstAreaFactory, secondAreaFactory, (_randomContext, firstArea, secondArea, x, z) => {
      const land = firstArea.get(x, z);
      const ocean = secondArea.get(x, z);
      if (!isOcean(land)) {
        return land;
      }

      for (let offsetZ = -8; offsetZ <= 8; offsetZ += 4) {
        for (let offsetX = -8; offsetX <= 8; offsetX += 4) {
          if (!isOcean(firstArea.get(x + offsetX, z + offsetZ))) {
            if (ocean === 44) {
              return 45;
            }

            if (ocean === 10) {
              return 46;
            }
          }
        }
      }

      if (land === 24) {
        if (ocean === 45) {
          return 48;
        }
        if (ocean === 0) {
          return 24;
        }
        if (ocean === 46) {
          return 49;
        }
        if (ocean === 10) {
          return 50;
        }
      }

      return ocean;
    });
  },
};

function zoom(
  seed: number,
  parent: UnaryAreaTransformer,
  areaFactory: AreaFactory,
  count: number,
  contextFactory: (seed: number) => LazyAreaContext,
): AreaFactory {
  let result = areaFactory;
  for (let index = 0; index < count; index++) {
    result = parent.run(contextFactory(seed + index), result);
  }

  return result;
}

function buildDefaultLayer(
  legacyBiomeInitLayer: boolean,
  biomeZooms: number,
  riverZooms: number,
  contextFactory: (seed: number) => LazyAreaContext,
): AreaFactory {
  let land = IslandLayer.run(contextFactory(1));
  land = ZoomLayer.FUZZY.run(contextFactory(2000), land);
  land = AddIslandLayer.run(contextFactory(1), land);
  land = ZoomLayer.NORMAL.run(contextFactory(2001), land);
  land = AddIslandLayer.run(contextFactory(2), land);
  land = AddIslandLayer.run(contextFactory(50), land);
  land = AddIslandLayer.run(contextFactory(70), land);
  land = RemoveTooMuchOceanLayer.run(contextFactory(2), land);

  let ocean = OceanLayer.run(contextFactory(2));
  ocean = zoom(2001, ZoomLayer.NORMAL, ocean, 6, contextFactory);

  land = AddSnowLayer.run(contextFactory(2), land);
  land = AddIslandLayer.run(contextFactory(3), land);
  land = AddEdgeLayerCoolWarm.run(contextFactory(2), land);
  land = AddEdgeLayerHeatIce.run(contextFactory(2), land);
  land = AddEdgeLayerIntroduceSpecial.run(contextFactory(3), land);
  land = ZoomLayer.NORMAL.run(contextFactory(2002), land);
  land = ZoomLayer.NORMAL.run(contextFactory(2003), land);
  land = AddIslandLayer.run(contextFactory(4), land);
  land = AddMushroomIslandLayer.run(contextFactory(5), land);
  land = AddDeepOceanLayer.run(contextFactory(4), land);
  land = zoom(1000, ZoomLayer.NORMAL, land, 0, contextFactory);

  let rivers = zoom(1000, ZoomLayer.NORMAL, land, 0, contextFactory);
  rivers = RiverInitLayer.run(contextFactory(100), rivers);

  let biomes = new BiomeInitLayer(legacyBiomeInitLayer).run(contextFactory(200), land);
  biomes = RareBiomeLargeLayer.run(contextFactory(1001), biomes);
  biomes = zoom(1000, ZoomLayer.NORMAL, biomes, 2, contextFactory);
  biomes = BiomeEdgeLayer.run(contextFactory(1000), biomes);

  const riverMixSource = zoom(1000, ZoomLayer.NORMAL, rivers, 2, contextFactory);
  biomes = RegionHillsLayer.run(contextFactory(1000), biomes, riverMixSource);

  rivers = zoom(1000, ZoomLayer.NORMAL, rivers, 2, contextFactory);
  rivers = zoom(1000, ZoomLayer.NORMAL, rivers, riverZooms, contextFactory);
  rivers = RiverLayer.run(contextFactory(1), rivers);
  rivers = SmoothLayer.run(contextFactory(1000), rivers);

  biomes = RareBiomeSpotLayer.run(contextFactory(1001), biomes);

  for (let index = 0; index < biomeZooms; index++) {
    biomes = ZoomLayer.NORMAL.run(contextFactory(1000 + index), biomes);
    if (index === 0) {
      biomes = AddIslandLayer.run(contextFactory(3), biomes);
    }

    if (index === 1 || biomeZooms === 1) {
      biomes = ShoreLayer.run(contextFactory(1000), biomes);
    }
  }

  biomes = SmoothLayer.run(contextFactory(1000), biomes);
  biomes = RiverMixerLayer.run(contextFactory(100), biomes, rivers);
  return OceanMixerLayer.run(contextFactory(100), biomes, ocean);
}

export function buildOverworldBiomeArea(seed: bigint, legacyBiomeInitLayer: boolean, biomeZooms: number, riverZooms: number): Area {
  const areaFactory = buildDefaultLayer(legacyBiomeInitLayer, biomeZooms, riverZooms, (salt) => new LazyAreaContext(25, seed, salt));
  return areaFactory();
}
