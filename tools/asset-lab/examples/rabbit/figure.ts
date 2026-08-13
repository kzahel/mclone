import { figure } from "../../src/dsl";

// A box-only rabbit using the vanilla cuboid anatomy: compact body, separate
// haunches, long rear feet, short forelegs, upright ears, and a small tail.
export default figure("rabbit", ({
  mat,
  asciiTexture,
  part,
  box,
  clip,
  defaultClip,
  geometryException,
  metadata,
  walkCycle,
  bob,
  contactSwing,
  followThrough,
}) => {
  metadata({
    bodyPlans: ["quadruped"],
    disposition: "passive",
    groups: ["animal"],
    habitats: ["land"],
    scale: "small",
    themes: ["rabbit", "burrow", "garden", "herbivore", "temperate"],
  });
  for (const penetratingPart of [
    "body",
    "belly",
    "rump",
    "leg_fl",
    "leg_fr",
    "leg_bl",
    "leg_br",
  ]) {
    geometryException({
      rule: "ground-penetration",
      parts: [penetratingPart],
      reason: "The authored enter/emerge actions intentionally lower this part through the excavated burrow threshold; ordinary grounded clips remain above grade.",
    });
  }

  mat("fur", "#f1f0ec");
  mat("fur_shadow", "#d9d8d2");
  mat("fur_light", "#fbfbf9");
  mat("ear_inner", "#e7a9b4");
  mat("foot", "#e7e5df");

  asciiTexture("face", {
    palette: {
      ".": "#f1f0ec",
      "e": "#2a2622",
      "n": "#e08a98",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "........",
      "...nn...",
      "........",
      "........",
    ],
  });

  asciiTexture("muzzle_face", {
    palette: {
      ".": "#fbfbf9",
      "n": "#e08a98",
      "m": "#8a6468",
    },
    pixels: [
      "..nn..",
      "..nn..",
      ".m..m.",
      "......",
    ],
  });

  part("body", box({
    at: [0, 0.68, 0.06],
    rot: [-7, 0, 0],
    size: [0.68, 0.52, 0.88],
    material: "fur",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.12, 0.35],
    size: [0.72, 0.62, 0.5],
    material: "fur_shadow",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.27, -0.06],
    size: [0.5, 0.12, 0.62],
    material: "fur_light",
  }));

  part("head", box({
    parent: "body",
    at: [0, 0.2, -0.6],
    size: [0.48, 0.42, 0.42],
    material: "fur",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.28],
    size: [0.34, 0.18, 0.18],
    material: "fur_light",
    faces: {
      north: { texture: "muzzle_face" },
    },
  }));

  part("ear_l", box({
    parent: "head",
    at: [-0.13, 0.52, 0.03],
    rot: [4, 0, -8],
    size: [0.15, 0.68, 0.12],
    material: "fur",
    joint: { pivot: [0, -0.34, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.13, 0.52, 0.03],
    rot: [4, 0, 8],
    size: [0.15, 0.68, 0.12],
    material: "fur",
    joint: { pivot: [0, -0.34, 0], axis: [1, 0, 0] },
  }));
  part("ear_inner_l", box({
    parent: "ear_l",
    at: [0, 0, -0.072],
    size: [0.08, 0.54, 0.025],
    material: "ear_inner",
  }));
  part("ear_inner_r", box({
    parent: "ear_r",
    at: [0, 0, -0.072],
    size: [0.08, 0.54, 0.025],
    material: "ear_inner",
  }));

  part("leg_fl", box({
    parent: "body",
    at: [-0.17, -0.44, -0.27],
    size: [0.14, 0.34, 0.16],
    material: "fur",
    joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
  }));
  part("leg_fr", box({
    parent: "body",
    at: [0.17, -0.44, -0.27],
    size: [0.14, 0.34, 0.16],
    material: "fur",
    joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
  }));
  part("paw_fl", box({
    parent: "leg_fl",
    at: [0, -0.19, -0.04],
    size: [0.18, 0.1, 0.24],
    material: "foot",
  }));
  part("paw_fr", box({
    parent: "leg_fr",
    at: [0, -0.19, -0.04],
    size: [0.18, 0.1, 0.24],
    material: "foot",
  }));

  part("leg_bl", box({
    parent: "body",
    at: [-0.26, -0.28, 0.3],
    rot: [-18, 0, 0],
    size: [0.28, 0.5, 0.42],
    material: "fur_shadow",
    joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
  }));
  part("leg_br", box({
    parent: "body",
    at: [0.26, -0.28, 0.3],
    rot: [-18, 0, 0],
    size: [0.28, 0.5, 0.42],
    material: "fur_shadow",
    joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
  }));
  part("foot_bl", box({
    parent: "leg_bl",
    at: [0, -0.35, -0.15],
    size: [0.24, 0.1, 0.54],
    material: "foot",
  }));
  part("foot_br", box({
    parent: "leg_br",
    at: [0, -0.35, -0.15],
    size: [0.24, 0.1, 0.54],
    material: "foot",
  }));

  part("tail", box({
    parent: "rump",
    at: [0, 0.04, 0.34],
    rot: [-12, 0, 0],
    size: [0.24, 0.24, 0.24],
    material: "fur_light",
  }));

  walkCycle("hop", {
    label: "Meadow hop",
    role: "locomotion",
    fps: 24,
    duration: 0.66,
    samples: 21,
    loop: true,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.8,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("body", { axis: "y", amount: 0.06, center: 0.06, phase: 0.5 }),
      contactSwing("leg_fl", { axis: "x", degrees: 24, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_fr", { axis: "x", degrees: 24, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_bl", { axis: "x", degrees: 30, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_br", { axis: "x", degrees: 30, phase: 0.75, stanceRatio: 0.5 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 16, overshoot: 0.85, lag: 0.16 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 16, overshoot: 0.85, lag: 0.16 }),
    ],
  });

  walkCycle("flee", {
    label: "Burrow flight",
    role: "locomotion",
    fps: 30,
    duration: 0.46,
    samples: 17,
    loop: true,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 1.12,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("body", { axis: "y", amount: 0.14, center: 0.12, phase: 0.5 }),
      contactSwing("leg_fl", { axis: "x", degrees: 38, phase: 0.75, stanceRatio: 0.38 }),
      contactSwing("leg_fr", { axis: "x", degrees: 38, phase: 0.75, stanceRatio: 0.38 }),
      contactSwing("leg_bl", { axis: "x", degrees: 48, phase: 0.75, stanceRatio: 0.38 }),
      contactSwing("leg_br", { axis: "x", degrees: 48, phase: 0.75, stanceRatio: 0.38 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 24, overshoot: 0.9, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 24, overshoot: 0.9, lag: 0.1 }),
    ],
  });

  clip("idle", {
    label: "Wary watch",
    role: "idle",
    fps: 20,
    loop: true,
    keys: [
      ["body", 0, { at: [0, 0, 0] }],
      ["body", 1.4, { at: [0, 0.012, 0] }],
      ["body", 2.8, { at: [0, 0, 0] }],
      ["head", 0, { rot: [0, -4, 0] }],
      ["head", 1.4, { rot: [-3, 5, 0] }],
      ["head", 2.8, { rot: [0, -4, 0] }],
      ["ear_l", 0, { rot: [0, 0, 0] }],
      ["ear_l", 0.55, { rot: [-11, 0, 8] }],
      ["ear_l", 1.1, { rot: [0, 0, 0] }],
      ["ear_l", 2.8, { rot: [0, 0, 0] }],
      ["ear_r", 0, { rot: [0, 0, 0] }],
      ["ear_r", 1.7, { rot: [-9, 0, -7] }],
      ["ear_r", 2.25, { rot: [0, 0, 0] }],
      ["ear_r", 2.8, { rot: [0, 0, 0] }],
      ["tail", 0, { rot: [0, 0, 0] }],
      ["tail", 1.4, { rot: [0, 0, 7] }],
      ["tail", 2.8, { rot: [0, 0, 0] }],
    ],
  });

  clip("forage", {
    label: "Ground forage",
    role: "idle",
    fps: 20,
    loop: true,
    keys: [
      ["body", 0, { at: [0, -0.04, 0], rot: [9, 0, 0] }],
      ["body", 0.7, { at: [0, -0.055, 0], rot: [11, 0, 0] }],
      ["body", 1.4, { at: [0, -0.04, 0], rot: [9, 0, 0] }],
      ["head", 0, { rot: [32, -3, 0] }],
      ["head", 0.35, { rot: [38, 3, 0] }],
      ["head", 0.7, { rot: [31, 0, 0] }],
      ["head", 1.05, { rot: [37, -3, 0] }],
      ["head", 1.4, { rot: [32, -3, 0] }],
      ["ear_l", 0, { rot: [-13, 0, 4] }],
      ["ear_l", 1.4, { rot: [-13, 0, 4] }],
      ["ear_r", 0, { rot: [-9, 0, -4] }],
      ["ear_r", 1.4, { rot: [-9, 0, -4] }],
    ],
  });

  clip("dig", {
    label: "Bank excavation",
    role: "action",
    fps: 24,
    loop: true,
    keys: [
      ["body", 0, { at: [0, 0.02, 0.03], rot: [10, 0, 0] }],
      ["body", 0.26, { at: [0, -0.02, -0.025], rot: [17, 0, 0] }],
      ["body", 0.52, { at: [0, 0.02, 0.03], rot: [10, 0, 0] }],
      ["head", 0, { rot: [18, 0, 0] }],
      ["head", 0.26, { rot: [27, 0, 0] }],
      ["head", 0.52, { rot: [18, 0, 0] }],
      ["leg_fl", 0, { rot: [-38, 0, 0] }],
      ["leg_fl", 0.13, { rot: [42, 0, 0] }],
      ["leg_fl", 0.39, { rot: [-38, 0, 0] }],
      ["leg_fl", 0.52, { rot: [-38, 0, 0] }],
      ["leg_fr", 0, { rot: [42, 0, 0] }],
      ["leg_fr", 0.13, { rot: [-38, 0, 0] }],
      ["leg_fr", 0.39, { rot: [42, 0, 0] }],
      ["leg_fr", 0.52, { rot: [42, 0, 0] }],
      ["ear_l", 0, { rot: [-22, 0, 4] }],
      ["ear_l", 0.52, { rot: [-22, 0, 4] }],
      ["ear_r", 0, { rot: [-22, 0, -4] }],
      ["ear_r", 0.52, { rot: [-22, 0, -4] }],
    ],
  });

  clip("enter_burrow", {
    label: "Enter burrow",
    role: "action",
    nextClip: "idle",
    fps: 24,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["body", 0.45, { at: [0, -0.18, -0.2], rot: [12, 0, 0] }],
      ["body", 0.9, { at: [0, -0.52, -0.62], rot: [18, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.9, { rot: [16, 0, 0] }],
      ["ear_l", 0, { rot: [0, 0, 0] }],
      ["ear_l", 0.9, { rot: [-38, 0, 2] }],
      ["ear_r", 0, { rot: [0, 0, 0] }],
      ["ear_r", 0.9, { rot: [-38, 0, -2] }],
    ],
  });

  clip("emerge", {
    label: "Emerge from burrow",
    role: "action",
    nextClip: "idle",
    fps: 24,
    loop: false,
    keys: [
      ["body", 0, { at: [0, -0.52, 0.62], rot: [18, 0, 0] }],
      ["body", 0.45, { at: [0, -0.18, 0.2], rot: [12, 0, 0] }],
      ["body", 0.9, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["head", 0, { rot: [16, 0, 0] }],
      ["head", 0.9, { rot: [0, 0, 0] }],
      ["ear_l", 0, { rot: [-38, 0, 2] }],
      ["ear_l", 0.9, { rot: [0, 0, 0] }],
      ["ear_r", 0, { rot: [-38, 0, -2] }],
      ["ear_r", 0.9, { rot: [0, 0, 0] }],
    ],
  });

  clip("courtship", {
    label: "Courtship circle",
    role: "action",
    fps: 20,
    loop: true,
    keys: [
      ["body", 0, { at: [0, 0, 0], rot: [0, -8, 0] }],
      ["body", 0.55, { at: [0, 0.07, -0.04], rot: [-4, 8, 0] }],
      ["body", 1.1, { at: [0, 0, 0], rot: [0, -8, 0] }],
      ["head", 0, { rot: [-5, 7, 0] }],
      ["head", 0.55, { rot: [-5, -7, 0] }],
      ["head", 1.1, { rot: [-5, 7, 0] }],
      ["tail", 0, { rot: [0, 0, -8] }],
      ["tail", 0.275, { rot: [0, 0, 8] }],
      ["tail", 0.55, { rot: [0, 0, -8] }],
      ["tail", 0.825, { rot: [0, 0, 8] }],
      ["tail", 1.1, { rot: [0, 0, -8] }],
    ],
  });

  defaultClip("idle");
});
