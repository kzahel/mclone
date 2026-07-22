export const CREATURE_GROUPS = [
  "animal",
  "plant",
  "fungus",
  "fantasy",
  "monster",
  "humanoid",
  "construct",
] as const;

export const CREATURE_BODY_PLANS = [
  "biped",
  "quadruped",
  "winged",
  "swimmer",
  "serpentine",
  "crawler",
  "blob",
  "rooted",
  "colony",
  "other",
] as const;

export const CREATURE_HABITATS = [
  "land",
  "water",
  "air",
  "underground",
] as const;

export const CREATURE_DISPOSITIONS = [
  "passive",
  "neutral",
  "hostile",
  "boss",
] as const;

export const CREATURE_SCALES = [
  "tiny",
  "small",
  "medium",
  "large",
  "giant",
] as const;

export type CreatureGroup = typeof CREATURE_GROUPS[number];
export type CreatureBodyPlan = typeof CREATURE_BODY_PLANS[number];
export type CreatureHabitat = typeof CREATURE_HABITATS[number];
export type CreatureDisposition = typeof CREATURE_DISPOSITIONS[number];
export type CreatureScale = typeof CREATURE_SCALES[number];

export interface CreatureMetadata {
  bodyPlans: CreatureBodyPlan[];
  disposition: CreatureDisposition;
  groups: CreatureGroup[];
  habitats: CreatureHabitat[];
  scale: CreatureScale;
  themes?: string[];
}

export function cloneCreatureMetadata(metadata: CreatureMetadata): CreatureMetadata {
  return {
    bodyPlans: [...metadata.bodyPlans],
    disposition: metadata.disposition,
    groups: [...metadata.groups],
    habitats: [...metadata.habitats],
    scale: metadata.scale,
    ...(metadata.themes === undefined ? {} : { themes: [...metadata.themes] }),
  };
}

export function validateCreatureMetadata(value: unknown, label = "metadata"): string[] {
  if (!isRecord(value)) {
    return [`${label} must be an object`];
  }
  const errors: string[] = [];
  validateEnumArray(value, "groups", CREATURE_GROUPS, label, errors);
  validateEnumArray(value, "bodyPlans", CREATURE_BODY_PLANS, label, errors);
  validateEnumArray(value, "habitats", CREATURE_HABITATS, label, errors);
  validateEnumValue(value, "disposition", CREATURE_DISPOSITIONS, label, errors);
  validateEnumValue(value, "scale", CREATURE_SCALES, label, errors);

  if (value.themes !== undefined) {
    if (!Array.isArray(value.themes)) {
      errors.push(`${label} themes must be an array`);
    } else {
      const seen = new Set<string>();
      for (const [index, theme] of value.themes.entries()) {
        if (typeof theme !== "string" || !/^[a-z0-9][a-z0-9-]*$/u.test(theme)) {
          errors.push(`${label} theme ${index} must be a lowercase tag`);
          continue;
        }
        if (seen.has(theme)) {
          errors.push(`${label} themes contain duplicate '${theme}'`);
        }
        seen.add(theme);
      }
    }
  }
  return errors;
}

function validateEnumArray<const T extends readonly string[]>(
  value: Record<string, unknown>,
  key: string,
  allowed: T,
  label: string,
  errors: string[],
): void {
  const entries = value[key];
  if (!Array.isArray(entries) || entries.length === 0) {
    errors.push(`${label} ${key} must be a nonempty array`);
    return;
  }
  const seen = new Set<string>();
  for (const [index, entry] of entries.entries()) {
    if (typeof entry !== "string" || !allowed.includes(entry)) {
      errors.push(`${label} ${key} entry ${index} '${String(entry)}' is invalid`);
      continue;
    }
    if (seen.has(entry)) {
      errors.push(`${label} ${key} contains duplicate '${entry}'`);
    }
    seen.add(entry);
  }
}

function validateEnumValue<const T extends readonly string[]>(
  value: Record<string, unknown>,
  key: string,
  allowed: T,
  label: string,
  errors: string[],
): void {
  const entry = value[key];
  if (typeof entry !== "string" || !allowed.includes(entry)) {
    errors.push(`${label} ${key} '${String(entry)}' is invalid`);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
