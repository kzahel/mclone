import {
  cloneCreatureMetadata,
  type CreatureBodyPlan,
  type CreatureGroup,
  type CreatureHabitat,
  type CreatureMetadata,
} from "./creature-metadata";
import type { FigureAsset, LocomotionKind } from "./dsl";

const CRAWLER_FIGURES = new Set([
  "ant",
  "bee",
  "centipede",
  "coconut_crab",
  "crab",
  "dragonfly",
  "earwig",
  "grasshopper",
  "horseshoe_crab",
  "ladybug",
  "leaf_insect",
  "lobster",
  "mantis_shrimp",
  "praying_mantis",
  "roly_poly",
  "scorpion",
  "shrimp",
  "spider",
  "stag_beetle",
  "starfish",
]);

const LAND_AND_WATER_FIGURES = new Set([
  "beaver",
  "capybara",
  "crocodile",
  "flamingo",
  "frog",
  "great_blue_heron",
  "harbor_seal",
  "hippopotamus",
  "mallard_duck",
  "marabou_stork",
  "pelican",
  "penguin",
  "platypus",
  "river_otter",
  "salamander",
  "sea_turtle",
  "shoebill",
  "walrus",
]);

const AMPHIBIOUS_BODY_PLANS: Partial<Record<string, CreatureBodyPlan[]>> = {
  beaver: ["quadruped", "swimmer"],
  capybara: ["quadruped", "swimmer"],
  crocodile: ["quadruped", "swimmer"],
  frog: ["quadruped", "swimmer"],
  harbor_seal: ["quadruped", "swimmer"],
  hippopotamus: ["quadruped", "swimmer"],
  mallard_duck: ["biped", "winged", "swimmer"],
  pelican: ["biped", "winged", "swimmer"],
  penguin: ["biped", "swimmer"],
  platypus: ["quadruped", "swimmer"],
  river_otter: ["quadruped", "swimmer"],
  sea_turtle: ["quadruped", "swimmer"],
  walrus: ["quadruped", "swimmer"],
};

const GROUP_OVERRIDES: Partial<Record<string, CreatureGroup[]>> = {
  bearfolk: ["fantasy", "humanoid"],
  lionfolk: ["fantasy", "humanoid"],
  player: ["humanoid"],
  upright_bear: ["animal", "humanoid"],
};

/**
 * Returns authored classifications when present. Older canonical figures stay
 * catalogue-compatible through deterministic inference until they are edited
 * and can adopt explicit metadata without a flag-day source migration.
 */
export function creatureMetadataForCatalog(asset: FigureAsset): CreatureMetadata {
  if (asset.metadata !== undefined) {
    return cloneCreatureMetadata(asset.metadata);
  }

  const locomotionKinds = new Set(
    Object.values(asset.clips).flatMap((clip) =>
      clip.locomotion === undefined ? [] : [clip.locomotion.kind]
    ),
  );
  const bodyPlans = new Set<CreatureBodyPlan>();
  const habitats = new Set<CreatureHabitat>();
  for (const kind of locomotionKinds) {
    bodyPlans.add(bodyPlanForLocomotion(kind));
    for (const habitat of habitatsForLocomotion(kind)) {
      habitats.add(habitat);
    }
  }

  if (CRAWLER_FIGURES.has(asset.name)) {
    bodyPlans.delete("quadruped");
    bodyPlans.add("crawler");
  }
  for (const bodyPlan of AMPHIBIOUS_BODY_PLANS[asset.name] ?? []) {
    bodyPlans.add(bodyPlan);
  }
  if (LAND_AND_WATER_FIGURES.has(asset.name)) {
    habitats.add("land");
    habitats.add("water");
  }
  if (bodyPlans.size === 0) {
    bodyPlans.add("other");
  }
  if (habitats.size === 0) {
    habitats.add("land");
  }

  return {
    bodyPlans: [...bodyPlans].sort(),
    disposition: "neutral",
    groups: [...(GROUP_OVERRIDES[asset.name] ?? ["animal"])],
    habitats: [...habitats].sort(),
    scale: "medium",
  };
}

function bodyPlanForLocomotion(kind: LocomotionKind): CreatureBodyPlan {
  switch (kind) {
    case "biped-walk": return "biped";
    case "quadruped-walk": return "quadruped";
    case "slither": return "serpentine";
    case "swim": return "swimmer";
    case "wing-flap": return "winged";
  }
}

function habitatsForLocomotion(kind: LocomotionKind): CreatureHabitat[] {
  switch (kind) {
    case "swim": return ["water"];
    case "wing-flap": return ["air", "land"];
    case "biped-walk":
    case "quadruped-walk":
    case "slither":
      return ["land"];
  }
}
