export const FAST_VISUAL_PROBE_PARAMS = {
  viewDistance: "1",
  renderDistance: "112",
  lightingMode: "none",
  liquidSimulationMode: "none",
} as const;

export const FAST_LIQUID_VISUAL_PROBE_PARAMS = {
  ...FAST_VISUAL_PROBE_PARAMS,
  liquidSimulationMode: "vanilla17",
} as const;
