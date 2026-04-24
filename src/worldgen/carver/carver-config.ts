import type { SimpleRandomSource } from "../prng/simple-random-source.ts";

export interface CarverContext {
  readonly minY: number;
  readonly genDepth: number;
  readonly seaLevel?: number;
}

export interface VerticalAnchor {
  resolveY(context: CarverContext): number;
}

export interface HeightProvider {
  sample(random: SimpleRandomSource, context: CarverContext): number;
}

export interface FloatProvider {
  sample(random: SimpleRandomSource): number;
}

export interface CarverConfiguration {
  readonly probability: number;
  readonly y: HeightProvider;
  readonly yScale: FloatProvider;
  readonly lavaLevel: VerticalAnchor;
  readonly aquifersEnabled: boolean;
}

export interface CaveCarverConfiguration extends CarverConfiguration {
  readonly horizontalRadiusMultiplier: FloatProvider;
  readonly verticalRadiusMultiplier: FloatProvider;
  readonly floorLevel: FloatProvider;
}

export interface CanyonShapeConfiguration {
  readonly distanceFactor: FloatProvider;
  readonly thickness: FloatProvider;
  readonly widthSmoothness: number;
  readonly horizontalRadiusFactor: FloatProvider;
  readonly verticalRadiusDefaultFactor: number;
  readonly verticalRadiusCenterFactor: number;
}

export interface CanyonCarverConfiguration extends CarverConfiguration {
  readonly verticalRotation: FloatProvider;
  readonly shape: CanyonShapeConfiguration;
}

function f32(value: number): number {
  return Math.fround(value);
}

export function absolute(value: number): VerticalAnchor {
  return {
    resolveY: () => value,
  };
}

export function aboveBottom(value: number): VerticalAnchor {
  return {
    resolveY: (context) => context.minY + value,
  };
}

export function belowTop(value: number): VerticalAnchor {
  return {
    resolveY: (context) => context.minY + context.genDepth - 1 - value,
  };
}

export function bottom(): VerticalAnchor {
  return aboveBottom(0);
}

export function top(): VerticalAnchor {
  return belowTop(0);
}

export function constantFloat(value: number): FloatProvider {
  const sampled = f32(value);
  return {
    sample: () => sampled,
  };
}

export function uniformFloat(minInclusive: number, maxExclusive: number): FloatProvider {
  const minValue = f32(minInclusive);
  const delta = f32(maxExclusive - minInclusive);
  return {
    sample: (random) => f32((random.nextFloat() * delta) + minValue),
  };
}

export function trapezoidFloat(min: number, max: number, plateau: number): FloatProvider {
  const minValue = f32(min);
  const span = f32(max - min);
  const plateauWidth = f32(plateau);
  const slopeWidth = f32((span - plateauWidth) / 2.0);
  const upperWidth = f32(span - slopeWidth);
  return {
    sample: (random) => f32(minValue + (random.nextFloat() * upperWidth) + (random.nextFloat() * slopeWidth)),
  };
}

export function biasedToBottomHeight(
  minInclusive: VerticalAnchor,
  maxInclusive: VerticalAnchor,
  inner: number,
): HeightProvider {
  return {
    sample: (random, context) => {
      const minValue = minInclusive.resolveY(context);
      const maxValue = maxInclusive.resolveY(context);
      const span = ((maxValue - minValue) - inner) + 1;
      if (span <= 0) {
        return minValue;
      }

      const bias = random.nextInt(span);
      return random.nextInt(bias + inner) + minValue;
    },
  };
}

export function uniformHeight(minInclusive: VerticalAnchor, maxInclusive: VerticalAnchor): HeightProvider {
  return {
    sample: (random, context) => {
      const minValue = minInclusive.resolveY(context);
      const maxValue = maxInclusive.resolveY(context);
      if (minValue > maxValue) {
        return minValue;
      }

      return minValue + random.nextInt((maxValue - minValue) + 1);
    },
  };
}

export function trapezoidHeight(minInclusive: VerticalAnchor, maxInclusive: VerticalAnchor, plateau = 0): HeightProvider {
  return {
    sample: (random, context) => {
      const minValue = minInclusive.resolveY(context);
      const maxValue = maxInclusive.resolveY(context);
      if (minValue > maxValue) {
        return minValue;
      }

      const span = maxValue - minValue;
      if (plateau >= span) {
        return minValue + random.nextInt(span + 1);
      }

      const lowerSpan = Math.trunc((span - plateau) / 2);
      const upperSpan = span - lowerSpan;
      return minValue + random.nextInt(upperSpan + 1) + random.nextInt(lowerSpan + 1);
    },
  };
}
