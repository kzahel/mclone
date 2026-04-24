import { describe, expect, test } from "vitest";

import { LightLayer, getLightLayerSurrounding } from "../../src/world/level/light-layer";
import {
  LIGHT_SECTION_PADDING,
  createLightSectionRange,
  createLightSectionRangeFromWorldSections,
  getLightSectionCount,
  getLightSectionIndex,
  getLightSectionYFromIndex,
  getMaxLightSection,
  getMinLightSection,
} from "../../src/world/level/light-section";

describe("LightLayer", () => {
  test("matches vanilla surrounding source levels", () => {
    expect(getLightLayerSurrounding(LightLayer.SKY)).toBe(15);
    expect(getLightLayerSurrounding(LightLayer.BLOCK)).toBe(0);
  });
});

describe("light section range helpers", () => {
  const level = {
    getSectionsCount: () => 16,
    getMinSection: () => 0,
  };

  test("adds vanilla one-section padding above and below the world", () => {
    expect(LIGHT_SECTION_PADDING).toBe(1);
    expect(getLightSectionCount(level)).toBe(18);
    expect(getMinLightSection(level)).toBe(-1);
    expect(getMaxLightSection(level)).toBe(17);
    expect(createLightSectionRange(level)).toEqual({
      minLightSection: -1,
      maxLightSection: 17,
      lightSectionCount: 18,
    });
  });

  test("supports negative section worlds", () => {
    expect(createLightSectionRangeFromWorldSections(-4, 24)).toEqual({
      minLightSection: -5,
      maxLightSection: 21,
      lightSectionCount: 26,
    });
  });

  test("maps section Y values to light packet-style indices", () => {
    expect(getLightSectionIndex(-1, -1)).toBe(0);
    expect(getLightSectionIndex(0, -1)).toBe(1);
    expect(getLightSectionIndex(16, -1)).toBe(17);
    expect(getLightSectionYFromIndex(17, -1)).toBe(16);
  });
});
