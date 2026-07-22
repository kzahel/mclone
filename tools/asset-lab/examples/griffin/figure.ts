import { figure } from "../../src/dsl";

// An anatomy-led eagle/lion griffin: eagle head, feathered chest, broad wings,
// and taloned forelegs join directly to a lion barrel and feline hindquarters.
export default figure("griffin", ({
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
    bodyPlans: ["quadruped", "winged"],
    disposition: "neutral",
    groups: ["animal", "fantasy"],
    habitats: ["land", "air"],
    scale: "large",
    themes: ["eagle", "griffin", "hybrid", "lion", "mythic", "raptor"],
  });

  mat("lion", "#bd7f35");
  mat("lion_light", "#dda95c");
  mat("lion_dark", "#5a3721");
  mat("feather", "#5a3a29");
  mat("feather_light", "#8b6242");
  mat("feather_dark", "#2b211b");
  mat("head_feather", "#e7dfcb");
  mat("beak", "#d5a232");
  mat("talon", "#906a24");
  mat("paw", "#3f291d");

  asciiTexture("eagle_face", {
    palette: { ".": "#e7dfcb", "e": "#d9b843", "p": "#171411", "s": "#bfb69f" },
    pixels: [
      "ssssssssss",
      "s........s",
      "s.eppppe.s",
      "s.eppppe.s",
      "..........",
      "...ssss...",
      "..........",
      "..........",
    ],
  });
  asciiTexture("wing_feathers", {
    palette: { ".": "#5a3a29", "l": "#8b6242", "d": "#2b211b" },
    pixels: [
      "llllllllllll",
      "l..........l",
      "ll..ll..llll",
      "..ll..ll....",
      "ddd....ddddd",
      "d..dddd....d",
      "dd......dddd",
      "dddddddddddd",
    ],
  });
  asciiTexture("talon_front", {
    palette: { "t": "#906a24", "c": "#1c1711" },
    pixels: ["tttttttt", "tcctcctt", "cccccccc"],
  });

  part("body", box({
    at: [0, 1.05, 0.08],
    size: [1.02, 0.66, 1.58],
    material: "lion",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.04, 0.62],
    size: [1.06, 0.66, 0.54],
    material: "lion_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.36, 0.04],
    size: [0.76, 0.12, 1.06],
    material: "lion_dark",
  }));
  part("feather_chest", box({
    parent: "body",
    at: [0, 0.05, -0.69],
    size: [1.08, 0.72, 0.5],
    material: "feather_light",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.3, -0.85],
    rot: [-18, 0, 0],
    size: [0.6, 0.62, 0.46],
    material: "head_feather",
    joint: { pivot: [0, -0.28, 0.15], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.24, -0.35],
    rot: [12, 0, 0],
    size: [0.56, 0.5, 0.5],
    material: "head_feather",
    faces: { north: { texture: "eagle_face" } },
  }));
  part("beak_upper", box({
    parent: "head",
    at: [0, -0.04, -0.39],
    rot: [10, 0, 0],
    size: [0.3, 0.23, 0.3],
    material: "beak",
  }));
  part("beak_hook", box({
    parent: "beak_upper",
    at: [0, -0.15, -0.12],
    rot: [25, 0, 0],
    size: [0.2, 0.17, 0.16],
    material: "talon",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_tuft_${side}`, box({
      parent: "head",
      at: [sign * 0.22, 0.27, 0.08],
      rot: [-7, 0, sign * 16],
      size: [0.12, 0.22, 0.12],
      material: "feather_dark",
    }));
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.61, 0.2, 0.06],
      rot: [0, sign * -7, sign * 12],
      size: [0.88, 0.12, 1.06],
      material: "feather",
      faces: { up: { texture: "wing_feathers" }, down: { texture: "wing_feathers" } },
      joint: { pivot: [sign * -0.4, 0, -0.22], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [sign * 0.63, -0.01, 0.08],
      rot: [0, sign * -9, 0],
      size: [0.55, 0.09, 0.9],
      material: "feather_dark",
      faces: { up: { texture: "wing_feathers" }, down: { texture: "wing_feathers" } },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.34, -0.49],
    ["fr", 0.34, -0.49],
  ] as const) {
    part(`foreleg_${suffix}`, box({
      parent: "body",
      at: [x, -0.55, z],
      size: [0.2, 0.64, 0.22],
      material: "beak",
      joint: { pivot: [0, 0.31, 0], axis: [1, 0, 0] },
    }));
    part(`talon_${suffix}`, box({
      parent: `foreleg_${suffix}`,
      at: [0, -0.39, -0.08],
      size: [0.34, 0.13, 0.42],
      material: "talon",
      faces: { north: { texture: "talon_front" } },
    }));
  }
  for (const [suffix, x, z] of [
    ["bl", -0.36, 0.49],
    ["br", 0.36, 0.49],
  ] as const) {
    part(`hindleg_${suffix}`, box({
      parent: "body",
      at: [x, -0.55, z],
      size: [0.25, 0.64, 0.26],
      material: "lion",
      joint: { pivot: [0, 0.31, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `hindleg_${suffix}`,
      at: [0, -0.39, -0.06],
      size: [0.32, 0.13, 0.4],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.28, 0.9],
    rot: [40, 0, 0],
    size: [0.13, 0.76, 0.14],
    material: "lion",
    joint: { pivot: [0, 0.36, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.55, -0.05],
    rot: [-10, 0, 0],
    size: [0.28, 0.3, 0.27],
    material: "lion_dark",
  }));

  quadrupedWalk("talon_lope", {
    label: "Talon lope",
    fps: 20,
    duration: 1.08,
    cycleDistance: 0.96,
    gait: "walk",
    loop: true,
    samples: 23,
    contactParts: {
      frontLeft: "talon_fl",
      frontRight: "talon_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.013,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "foreleg_fl",
      frontRight: "foreleg_fr",
      backLeft: "hindleg_bl",
      backRight: "hindleg_br",
    },
    stanceRatio: 0.65,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      swing("wing_l", { axis: "z", degrees: 2.5, phase: 0.1 }),
      swing("wing_r", { axis: "z", degrees: -2.5, phase: 0.1 }),
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.14 }),
    ],
  });
  clip("wing_pounce", {
    label: "Wing-assisted pounce",
    role: "action",
    nextClip: "talon_lope",
    fps: 30,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["body", 0.32, { at: [0, -0.05, 0.05], rot: [-5, 0, 0] }],
      ["body", 0.72, { at: [0, 0.08, -0.05], rot: [7, 0, 0] }],
      ["body", 1.34, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_l", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_l", 0.32, { at: [-0.03, 0.04, 0], rot: [0, 0, -34] }],
      ["wing_l", 0.72, { at: [-0.08, 0.12, 0], rot: [0, 0, -58] }],
      ["wing_l", 1.34, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_r", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_r", 0.32, { at: [0.03, 0.04, 0], rot: [0, 0, 34] }],
      ["wing_r", 0.72, { at: [0.08, 0.12, 0], rot: [0, 0, 58] }],
      ["wing_r", 1.34, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["foreleg_fl", 0, { rot: [0, 0, 0] }],
      ["foreleg_fl", 0.32, { rot: [22, 0, -5] }],
      ["foreleg_fl", 0.72, { rot: [-36, 0, -8] }],
      ["foreleg_fl", 1.34, { rot: [0, 0, 0] }],
      ["foreleg_fr", 0, { rot: [0, 0, 0] }],
      ["foreleg_fr", 0.32, { rot: [22, 0, 5] }],
      ["foreleg_fr", 0.72, { rot: [-36, 0, 8] }],
      ["foreleg_fr", 1.34, { rot: [0, 0, 0] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.32, { rot: [-8, 0, 0] }],
      ["neck", 0.72, { rot: [12, 0, 0] }],
      ["neck", 1.34, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("talon_lope");
});
