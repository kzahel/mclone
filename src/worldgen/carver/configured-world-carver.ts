import type { CarverConfiguration } from "./carver-config.ts";
import type { WorldCarver } from "./world-carver.ts";

export interface ConfiguredWorldCarver<C extends CarverConfiguration = CarverConfiguration> {
  readonly worldCarver: WorldCarver<C>;
  readonly config: C;
}

export function configureWorldCarver<C extends CarverConfiguration>(
  worldCarver: WorldCarver<C>,
  config: C,
): ConfiguredWorldCarver<C> {
  return { worldCarver, config };
}
