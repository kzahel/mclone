import { figure } from "../../src/dsl";

// A heavy owlbear whose owl traits are anatomical rather than ornamental:
// broad facial disc, hooked beak, ear tufts, feathered neck and shoulders,
// bear barrel, thick legs, and oversized paws.
export default figure("owlbear", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
  quadrupedWalk,
}) => {
  metadata({
    bodyPlans: ["quadruped"],
    disposition: "neutral",
    groups: ["animal", "fantasy"],
    habitats: ["land"],
    scale: "large",
    themes: ["feathered", "forest", "mythic", "owlbear", "predator"],
  });

  mat("fur", "#68462f");
  mat("fur_light", "#956a45");
  mat("fur_dark", "#3e2c22");
  mat("feather", "#745841");
  mat("feather_light", "#b09570");
  mat("feather_dark", "#302923");
  mat("disc", "#d5c397");
  mat("beak", "#c79132");
  mat("claw", "#1e1a17");

  asciiTexture("owlbear_face", {
    palette: { ".": "#d5c397", "r": "#745841", "d": "#302923", "e": "#d8a63a", "b": "#1e1a17" },
    pixels: [
      "rrr......rrr",
      "rr.dd..dd.rr",
      "r.dee..eed.r",
      "..ddbbbbdd..",
      "....dddd....",
      "...rrddrr...",
      "..rrr..rrr..",
      "rrr......rrr",
    ],
  });
  asciiTexture("breast_feathers", {
    palette: { ".": "#745841", "l": "#b09570", "d": "#302923" },
    pixels: [
      "llddddddddll",
      "l.ddlllldd.l",
      "..llddddll..",
      "dd..llll..dd",
      "d.ll....ll.d",
      "..dddddddd..",
      "ll..dddd..ll",
    ],
  });
  asciiTexture("paw_claws", {
    palette: { ".": "#3e2c22", "c": "#1e1a17" },
    pixels: ["........", ".c.cc.c.", "cccccccc"],
  });

  part("body", box({
    at: [0, 1.1, 0.08],
    size: [1.14, 0.84, 1.5],
    material: "fur",
  }));
  part("shoulder_hump", box({
    parent: "body",
    at: [0, 0.42, -0.42],
    rot: [-4, 0, 0],
    size: [1.06, 0.34, 0.7],
    material: "feather_dark",
  }));
  part("breast_ruff", box({
    parent: "body",
    at: [0, -0.02, -0.8],
    size: [0.86, 0.7, 0.2],
    material: "feather",
    faces: { north: { texture: "breast_feathers" } },
    joint: { pivot: [0, 0.28, 0.06], axis: [1, 0, 0] },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.45, 0.06],
    size: [0.92, 0.12, 1.08],
    material: "fur_dark",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.2, -0.83],
    rot: [-9, 0, 0],
    size: [0.72, 0.52, 0.48],
    material: "feather",
    joint: { pivot: [0, -0.24, 0.16], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.14, -0.43],
    rot: [7, 0, 0],
    size: [0.86, 0.62, 0.58],
    material: "disc",
    faces: { north: { texture: "owlbear_face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.09, -0.38],
    rot: [15, 0, 0],
    size: [0.2, 0.24, 0.2],
    material: "beak",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_tuft_${side}`, box({
      parent: "head",
      at: [sign * 0.3, 0.38, 0.02],
      rot: [0, 0, sign * 15],
      size: [0.14, 0.3, 0.14],
      material: "feather_dark",
    }));
    part(`mantle_${side}`, box({
      parent: "body",
      at: [sign * 0.55, 0.16, -0.35],
      rot: [0, sign * -5, sign * 5],
      size: [0.24, 0.58, 0.72],
      material: side === "l" ? "feather" : "feather_light",
      faces: { east: { texture: "breast_feathers" }, west: { texture: "breast_feathers" } },
      joint: { pivot: [sign * -0.09, 0.22, 0.16], axis: [0, 0, 1] },
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.38, -0.48, false],
    ["fr", 0.38, -0.48, false],
    ["bl", -0.39, 0.5, true],
    ["br", 0.39, 0.5, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.61, z],
      size: [rear ? 0.32 : 0.3, rear ? 0.52 : 0.5, rear ? 0.36 : 0.32],
      material: rear ? "fur_dark" : "feather_dark",
      joint: { pivot: [0, rear ? 0.25 : 0.24, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.32 : -0.31, -0.08],
      size: [rear ? 0.38 : 0.37, 0.13, rear ? 0.46 : 0.44],
      material: "fur_dark",
      faces: { north: { texture: "paw_claws" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.08, 0.82],
    rot: [38, 0, 0],
    size: [0.18, 0.22, 0.18],
    material: "fur_dark",
    joint: { pivot: [0, 0.09, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("lumber", {
    label: "Heavy lumber",
    fps: 16,
    duration: 1.38,
    cycleDistance: 0.68,
    gait: "walk",
    loop: true,
    samples: 25,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.011,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.74,
    swingDegrees: 14,
    tail: "tail",
    tailSwingDegrees: 3,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.42, lag: 0.16 }),
      followThrough("breast_ruff", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.12 }),
    ],
  });
  clip("feather_ruffle", {
    label: "Feather ruffle",
    role: "action",
    nextClip: "lumber",
    fps: 30,
    loop: false,
    keys: [
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.22, { rot: [-8, -8, -3] }],
      ["head", 0.42, { rot: [-5, 10, 4] }],
      ["head", 0.62, { rot: [-7, -6, -3] }],
      ["head", 1.12, { rot: [0, 0, 0] }],
      ["breast_ruff", 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["breast_ruff", 0.22, { at: [0, 0.03, -0.03], scale: [1.08, 1.1, 1.12] }],
      ["breast_ruff", 0.42, { at: [0, -0.01, -0.05], scale: [1.13, 1.05, 1.16] }],
      ["breast_ruff", 0.62, { at: [0, 0.02, -0.03], scale: [1.07, 1.09, 1.1] }],
      ["breast_ruff", 1.12, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["mantle_l", 0, { at: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["mantle_l", 0.22, { at: [-0.04, 0.02, 0], rot: [0, 0, -13], scale: [1.08, 1.08, 1.05] }],
      ["mantle_l", 0.42, { at: [-0.06, 0, 0], rot: [0, 0, -18], scale: [1.12, 1.04, 1.08] }],
      ["mantle_l", 0.62, { at: [-0.03, 0.02, 0], rot: [0, 0, -10], scale: [1.06, 1.08, 1.04] }],
      ["mantle_l", 1.12, { at: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["mantle_r", 0, { at: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["mantle_r", 0.22, { at: [0.04, 0.02, 0], rot: [0, 0, 13], scale: [1.08, 1.08, 1.05] }],
      ["mantle_r", 0.42, { at: [0.06, 0, 0], rot: [0, 0, 18], scale: [1.12, 1.04, 1.08] }],
      ["mantle_r", 0.62, { at: [0.03, 0.02, 0], rot: [0, 0, 10], scale: [1.06, 1.08, 1.04] }],
      ["mantle_r", 1.12, { at: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] }],
    ],
  });
  defaultClip("lumber");
});
