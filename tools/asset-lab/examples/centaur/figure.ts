import { figure } from "../../src/dsl";

// A bare, anatomy-led centaur: a human trunk grows directly from a horse's
// shoulders, with no saddle, clothing, weapon, or other silhouette shortcut.
export default figure("centaur", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
  quadrupedWalk,
  swing,
}) => {
  metadata({
    bodyPlans: ["quadruped", "biped"],
    disposition: "neutral",
    groups: ["animal", "fantasy", "humanoid"],
    habitats: ["land"],
    scale: "large",
    themes: ["centaur", "equine", "hybrid", "humanoid", "mythic"],
  });

  mat("coat", "#8a5631");
  mat("coat_light", "#b47a49");
  mat("coat_dark", "#50341f");
  mat("skin", "#b87855");
  mat("skin_light", "#d19a73");
  mat("hair", "#38251c");
  mat("hoof", "#211a17");

  asciiTexture("centaur_face", {
    palette: { ".": "#b87855", "e": "#211713", "h": "#38251c", "l": "#d19a73" },
    pixels: [
      "hhhhhhhhhh",
      "h........h",
      "h.ee..ee.h",
      "..ee..ee..",
      "....ll....",
      "...llll...",
      "..........",
      "..........",
    ],
  });
  asciiTexture("horse_flank", {
    palette: { ".": "#8a5631", "l": "#b47a49", "d": "#50341f" },
    pixels: [
      "dddddddddddd",
      "dlllllllllld",
      "l..........l",
      "............",
      "..llllll....",
      "............",
      "dddddddddddd",
    ],
  });
  asciiTexture("hoof_front", {
    palette: { "h": "#211a17", "d": "#0f0c0b" },
    pixels: ["hhhhhh", "hhddhh", "hhhhhh"],
  });

  part("horse_body", box({
    at: [0, 1.12, 0.04],
    size: [0.94, 0.68, 1.7],
    material: "coat",
    faces: {
      east: { texture: "horse_flank" },
      west: { texture: "horse_flank" },
    },
  }));
  part("horse_chest", box({
    parent: "horse_body",
    at: [0, 0.03, -0.72],
    size: [0.98, 0.72, 0.5],
    material: "coat_light",
  }));
  part("belly", box({
    parent: "horse_body",
    at: [0, -0.36, 0.1],
    size: [0.72, 0.1, 1.16],
    material: "coat_dark",
  }));

  part("human_waist", box({
    parent: "horse_body",
    at: [0, 0.5, -0.58],
    size: [0.44, 0.42, 0.46],
    material: "coat_light",
  }));
  part("human_torso", box({
    parent: "human_waist",
    at: [0, 0.48, -0.01],
    size: [0.62, 0.7, 0.38],
    material: "skin",
    joint: { pivot: [0, -0.33, 0], axis: [1, 0, 0] },
  }));
  part("chest_plane", box({
    parent: "human_torso",
    at: [0, 0.08, -0.21],
    size: [0.5, 0.28, 0.08],
    material: "skin_light",
  }));
  part("human_neck", box({
    parent: "human_torso",
    at: [0, 0.43, 0],
    size: [0.24, 0.24, 0.24],
    material: "skin",
  }));
  part("human_head", box({
    parent: "human_neck",
    at: [0, 0.3, -0.02],
    size: [0.48, 0.48, 0.46],
    material: "skin",
    faces: { north: { texture: "centaur_face" } },
    joint: { pivot: [0, -0.2, 0], axis: [0, 1, 0] },
  }));
  part("hair_cap", box({
    parent: "human_head",
    at: [0, 0.2, 0.02],
    size: [0.5, 0.12, 0.48],
    material: "hair",
  }));
  part("hair_back", box({
    parent: "human_head",
    at: [0, -0.02, 0.25],
    size: [0.4, 0.42, 0.12],
    material: "hair",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`arm_${side}`, box({
      parent: "human_torso",
      at: [sign * 0.4, 0.08, 0],
      size: [0.19, 0.66, 0.21],
      material: "skin",
      joint: { pivot: [0, 0.31, 0], axis: [1, 0, 0] },
    }));
    part(`hand_${side}`, box({
      parent: `arm_${side}`,
      at: [0, -0.4, -0.025],
      size: [0.21, 0.2, 0.23],
      material: "skin_light",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.31, -0.58, false],
    ["fr", 0.31, -0.58, false],
    ["bl", -0.33, 0.59, true],
    ["br", 0.33, 0.59, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "horse_body",
      at: [x, -0.58, z],
      size: [rear ? 0.22 : 0.19, rear ? 0.72 : 0.7, rear ? 0.23 : 0.2],
      material: rear ? "coat" : "coat_light",
      joint: { pivot: [0, rear ? 0.35 : 0.34, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.43 : -0.42, -0.02],
      size: [rear ? 0.25 : 0.23, 0.13, rear ? 0.3 : 0.28],
      material: "hoof",
      faces: { north: { texture: "hoof_front" } },
    }));
  }

  part("tail_root", box({
    parent: "horse_body",
    at: [0, 0.24, 0.9],
    rot: [34, 0, 0],
    size: [0.15, 0.62, 0.17],
    material: "hair",
    joint: { pivot: [0, 0.29, 0], axis: [0, 0, 1] },
  }));
  part("tail_fall", box({
    parent: "tail_root",
    at: [0, -0.49, -0.04],
    rot: [-12, 0, 0],
    size: [0.25, 0.5, 0.26],
    material: "hair",
  }));

  quadrupedWalk("four_beat_trot", {
    label: "Four-beat trot",
    fps: 20,
    duration: 1.12,
    cycleDistance: 1.02,
    gait: "walk",
    loop: true,
    samples: 23,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "horse_body",
    bodyBob: 0.012,
    bodyBobCenter: 0.013,
    head: "human_head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.66,
    swingDegrees: 18,
    tail: "tail_root",
    tailSwingDegrees: 7,
    tracks: [
      swing("arm_l", { axis: "x", degrees: 7, phase: 0.5 }),
      swing("arm_r", { axis: "x", degrees: 7, phase: 0 }),
      followThrough("human_torso", { source: "horse_body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.12 }),
    ],
  });
  clip("high_rear", {
    label: "High rear",
    role: "action",
    nextClip: "four_beat_trot",
    fps: 30,
    loop: false,
    keys: [
      ["horse_body", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["horse_body", 0.34, { at: [0, 0.11, 0], rot: [12, 0, 0] }],
      ["horse_body", 0.86, { at: [0, 0.12, 0], rot: [14, 0, 0] }],
      ["horse_body", 1.52, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["leg_fl", 0, { rot: [0, 0, 0] }],
      ["leg_fl", 0.34, { rot: [-45, 0, -7] }],
      ["leg_fl", 0.86, { rot: [-58, 0, -10] }],
      ["leg_fl", 1.52, { rot: [0, 0, 0] }],
      ["leg_fr", 0, { rot: [0, 0, 0] }],
      ["leg_fr", 0.34, { rot: [-38, 0, 7] }],
      ["leg_fr", 0.86, { rot: [-51, 0, 10] }],
      ["leg_fr", 1.52, { rot: [0, 0, 0] }],
      ["human_torso", 0, { rot: [0, 0, 0] }],
      ["human_torso", 0.34, { rot: [-9, 0, 0] }],
      ["human_torso", 0.86, { rot: [-13, 0, 0] }],
      ["human_torso", 1.52, { rot: [0, 0, 0] }],
      ["arm_l", 0, { rot: [0, 0, 0] }],
      ["arm_l", 0.34, { rot: [-18, 0, -28] }],
      ["arm_l", 0.86, { rot: [-25, 0, -42] }],
      ["arm_l", 1.52, { rot: [0, 0, 0] }],
      ["arm_r", 0, { rot: [0, 0, 0] }],
      ["arm_r", 0.34, { rot: [-18, 0, 28] }],
      ["arm_r", 0.86, { rot: [-25, 0, 42] }],
      ["arm_r", 1.52, { rot: [0, 0, 0] }],
      ["tail_root", 0, { rot: [0, 0, 0] }],
      ["tail_root", 0.34, { rot: [-12, 0, -5] }],
      ["tail_root", 0.86, { rot: [-18, 0, 7] }],
      ["tail_root", 1.52, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("four_beat_trot");
});
