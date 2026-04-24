export const LIGHT_SECTION_PADDING = 1;

export interface LevelHeightAccessorLike {
  getSectionsCount(): number;
  getMinSection(): number;
}

export interface LightSectionRange {
  readonly minLightSection: number;
  readonly maxLightSection: number;
  readonly lightSectionCount: number;
}

export function getLightSectionCount(level: LevelHeightAccessorLike): number {
  return level.getSectionsCount() + (LIGHT_SECTION_PADDING * 2);
}

export function getMinLightSection(level: LevelHeightAccessorLike): number {
  return level.getMinSection() - LIGHT_SECTION_PADDING;
}

export function getMaxLightSection(level: LevelHeightAccessorLike): number {
  return getMinLightSection(level) + getLightSectionCount(level);
}

export function createLightSectionRange(level: LevelHeightAccessorLike): LightSectionRange {
  const minLightSection = getMinLightSection(level);
  const lightSectionCount = getLightSectionCount(level);
  return {
    minLightSection,
    maxLightSection: minLightSection + lightSectionCount,
    lightSectionCount,
  };
}

export function createLightSectionRangeFromWorldSections(minSection: number, sectionCount: number): LightSectionRange {
  const minLightSection = minSection - LIGHT_SECTION_PADDING;
  const lightSectionCount = sectionCount + (LIGHT_SECTION_PADDING * 2);
  return {
    minLightSection,
    maxLightSection: minLightSection + lightSectionCount,
    lightSectionCount,
  };
}

export function getLightSectionIndex(sectionY: number, minLightSection: number): number {
  return sectionY - minLightSection;
}

export function getLightSectionYFromIndex(index: number, minLightSection: number): number {
  return minLightSection + index;
}
