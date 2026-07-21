import { figure } from "../../src/dsl";

// A box-only woolly llama with a deep fleece body, upright two-stage neck,
// long alert ears, narrow face, slim two-stage legs, and a curled woolly tail.
export default figure("llama", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("wool", "#d9c7a5");
  mat("wool_light", "#eee2c8");
  mat("wool_shadow", "#b89d76");
  mat("face", "#a87f57");
  mat("face_light", "#c49b70");
  mat("muzzle", "#d6b990");
  mat("hoof", "#40352c");
  mat("ear_inner", "#9a665d");

  asciiTexture("face_mask", {
    palette: { ".": "#a87f57", "l": "#d6b990", "e": "#211914", "w": "#eee2c8" },
    pixels: [
      "ww....ww",
      "w.e..e.w",
      "..e..e..",
      "..llll..",
      ".llllll.",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#d6b990", "n": "#4b392c" },
    pixels: ["........", ".nn..nn.", "..nnnn..", "........", "........"],
  });
  asciiTexture("fleece_side", {
    palette: { ".": "#d9c7a5", "l": "#eee2c8", "s": "#b89d76" },
    pixels: [
      "sllssllsslls",
      "llllllllllll",
      "ll..llll..ll",
      "llllllllllll",
      "llllllllllll",
      "sllssllsslls",
      ".ss..ss..ss.",
    ],
  });
  asciiTexture("neck_fleece", {
    palette: { ".": "#d9c7a5", "l": "#eee2c8", "s": "#b89d76" },
    pixels: [
      "slllls",
      "llllll",
      "ll..ll",
      "llllll",
      "slllls",
      "llllll",
      "ll..ll",
      "llllll",
      "slllls",
      "llllll",
    ],
  });

  part("body", box({
    at: [0, 1.42, 0.08],
    size: [0.94, 0.84, 1.36],
    material: "wool",
    faces: {
      east: { texture: "fleece_side" },
      west: { texture: "fleece_side" },
    },
  }));
  part("wool_top", box({
    parent: "body",
    at: [0, 0.49, 0.02],
    size: [0.82, 0.2, 1.16],
    material: "wool_light",
  }));
  part("chest_wool", box({
    parent: "body",
    at: [0, -0.03, -0.72],
    size: [0.86, 0.84, 0.28],
    material: "wool_light",
  }));
  part("belly_wool", box({
    parent: "body",
    at: [0, -0.46, 0.05],
    size: [0.78, 0.18, 1.04],
    material: "wool_shadow",
  }));
  part("neck_lower", box({
    parent: "body",
    at: [0, 0.62, -0.58],
    rot: [-8, 0, 0],
    size: [0.5, 1.05, 0.48],
    material: "wool",
    faces: {
      east: { texture: "neck_fleece" },
      west: { texture: "neck_fleece" },
    },
    joint: { pivot: [0, -0.5, 0.08], axis: [1, 0, 0] },
  }));
  part("neck_upper", box({
    parent: "neck_lower",
    at: [0, 0.64, -0.08],
    rot: [5, 0, 0],
    size: [0.4, 0.58, 0.4],
    material: "wool_light",
  }));
  part("head", box({
    parent: "neck_upper",
    at: [0, 0.39, -0.15],
    rot: [8, 0, 0],
    size: [0.44, 0.52, 0.5],
    material: "face",
    faces: { north: { texture: "face_mask" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.16, -0.38],
    size: [0.34, 0.26, 0.3],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("forelock", box({
    parent: "head",
    at: [0, 0.32, -0.04],
    size: [0.36, 0.2, 0.34],
    material: "wool_light",
  }));
  for (const [side, x] of [["l", -0.16], ["r", 0.16]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.42, 0.08],
      rot: [-5, 0, side === "l" ? -8 : 8],
      size: [0.13, 0.38, 0.11],
      material: "ear_inner",
      joint: { pivot: [0, -0.17, 0], axis: [1, 0, 0] },
    }));
    part(`ear_tip_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0.24, 0],
      size: [0.1, 0.14, 0.09],
      material: "face",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.31, -0.47],
    ["fr", 0.31, -0.47],
    ["bl", -0.31, 0.49],
    ["br", 0.31, 0.49],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.7, z],
      size: [0.19, 0.68, 0.2],
      material: "wool_shadow",
      joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.51, 0],
      size: [0.15, 0.42, 0.16],
      material: "face_light",
    }));
    part(`hoof_${suffix}`, box({
      parent: `shin_${suffix}`,
      at: [0, -0.27, -0.04],
      size: [0.22, 0.12, 0.28],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.24, 0.73],
    rot: [-42, 0, 0],
    size: [0.22, 0.38, 0.22],
    material: "wool_light",
    joint: { pivot: [0, -0.17, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.26, -0.03],
    size: [0.27, 0.22, 0.25],
    material: "wool",
  }));

  quadrupedWalk("stroll", {
    fps: 18,
    duration: 1.24,
    cycleDistance: 0.86,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.016,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.69,
    swingDegrees: 16,
    tail: "tail",
    tailSwingDegrees: 6,
    tracks: [
      followThrough("neck_lower", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.4, lag: 0.13 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.12 }),
    ],
  });
});
