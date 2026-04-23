import {
  aboveBottom,
  absolute,
  biasedToBottomHeight,
  constantFloat,
  trapezoidFloat,
  type CaveCarverConfiguration,
  type CanyonCarverConfiguration,
  uniformFloat,
} from "./carver-config.ts";
import { CaveWorldCarver } from "./cave-world-carver.ts";
import { CanyonWorldCarver } from "./canyon-world-carver.ts";
import { configureWorldCarver } from "./configured-world-carver.ts";

export const CAVE = configureWorldCarver(new CaveWorldCarver(), {
  probability: 0.14285715,
  y: biasedToBottomHeight(absolute(0), absolute(127), 8),
  yScale: constantFloat(0.5),
  lavaLevel: aboveBottom(10),
  aquifersEnabled: false,
  horizontalRadiusMultiplier: constantFloat(1.0),
  verticalRadiusMultiplier: constantFloat(1.0),
  floorLevel: constantFloat(-0.7),
} satisfies CaveCarverConfiguration);

export const OCEAN_CAVE = configureWorldCarver(new CaveWorldCarver(), {
  probability: 0.06666667,
  y: biasedToBottomHeight(absolute(0), absolute(127), 8),
  yScale: constantFloat(0.5),
  lavaLevel: aboveBottom(10),
  aquifersEnabled: false,
  horizontalRadiusMultiplier: constantFloat(1.0),
  verticalRadiusMultiplier: constantFloat(1.0),
  floorLevel: constantFloat(-0.7),
} satisfies CaveCarverConfiguration);

export const CANYON = configureWorldCarver(new CanyonWorldCarver(), {
  probability: 0.02,
  y: biasedToBottomHeight(absolute(20), absolute(67), 8),
  yScale: constantFloat(3.0),
  lavaLevel: aboveBottom(10),
  aquifersEnabled: false,
  verticalRotation: uniformFloat(-0.125, 0.125),
  shape: {
    distanceFactor: uniformFloat(0.75, 1.0),
    thickness: trapezoidFloat(0.0, 6.0, 2.0),
    widthSmoothness: 3,
    horizontalRadiusFactor: uniformFloat(0.75, 1.0),
    verticalRadiusDefaultFactor: 1.0,
    verticalRadiusCenterFactor: 0.0,
  },
} satisfies CanyonCarverConfiguration);

export const DEFAULT_OVERWORLD_AIR_CARVERS = [CAVE, CANYON] as const;
export const OCEAN_OVERWORLD_AIR_CARVERS = [OCEAN_CAVE, CANYON] as const;
