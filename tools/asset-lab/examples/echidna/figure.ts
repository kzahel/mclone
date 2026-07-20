import { figure } from "../../src/dsl";

// A box-only short-beaked echidna with a low domed spine coat, long narrow
// snout, powerful outward-set feet, and pale digging claws painted directly on
// the paw fronts. The compact walk stays deliberately close to the ground.
export default figure("echidna", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
}) => {
  mat("fur", "#4a382c");
  mat("fur_light", "#705744");
  mat("fur_dark", "#30261f");
  mat("spine", "#6a5941");
  mat("spine_light", "#c3ad7d");
  mat("spine_pale", "#e0cfa1");
  mat("belly", "#8e7053");
  mat("snout", "#624939");
  mat("nose", "#1d1916");
  mat("paw", "#3a2d25");

  asciiTexture("face", {
    palette: { ".": "#705744", "e": "#171411", "l": "#9a7b5d" },
    pixels: [
      "ll....ll",
      ".ee..ee.",
      "..llll..",
      "........",
      "........",
    ],
  });
  asciiTexture("snout_tip", {
    palette: { ".": "#624939", "n": "#1d1916" },
    pixels: [
      "........",
      "..nnnn..",
      ".nnnnnn.",
      "..nnnn..",
    ],
  });
  asciiTexture("spine_side", {
    palette: { ".": "#6a5941", "l": "#c3ad7d", "p": "#e0cfa1", "d": "#4a382c" },
    pixels: [
      "p.l.p.l.p.l.p",
      ".p.l.p.l.p.l.",
      "l.p.l.p.l.p.l",
      ".l.p.l.p.l.p.",
      "dd..dd..dd..d",
    ],
  });
  asciiTexture("spine_top", {
    palette: { ".": "#6a5941", "l": "#c3ad7d", "p": "#e0cfa1" },
    pixels: [
      "p..l..p..l..p",
      ".p..l..p..l..",
      "l..p..l..p..l",
      "..l..p..l..p.",
      "p..l..p..l..p",
    ],
  });
  asciiTexture("claw_front", {
    palette: { ".": "#3a2d25", "c": "#d8c59a" },
    pixels: [
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.5, 0.07],
    size: [0.78, 0.4, 1.02],
    material: "fur",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.25, -0.02],
    size: [0.58, 0.1, 0.72],
    material: "belly",
  }));
  part("spine_main", box({
    parent: "body",
    at: [0, 0.22, 0.08],
    size: [0.8, 0.4, 0.66],
    material: "spine",
    faces: {
      east: { texture: "spine_side" },
      west: { texture: "spine_side" },
      up: { texture: "spine_top" },
    },
  }));
  part("spine_front", box({
    parent: "body",
    at: [0, 0.14, -0.3],
    size: [0.7, 0.3, 0.32],
    material: "spine_light",
    faces: {
      east: { texture: "spine_side" },
      west: { texture: "spine_side" },
    },
  }));
  part("spine_rear", box({
    parent: "body",
    at: [0, 0.15, 0.39],
    size: [0.7, 0.32, 0.28],
    material: "spine",
    faces: {
      east: { texture: "spine_side" },
      west: { texture: "spine_side" },
    },
  }));
  part("spine_crown", box({
    parent: "body",
    at: [0, 0.43, 0.07],
    size: [0.56, 0.16, 0.48],
    material: "spine_pale",
    faces: { up: { texture: "spine_top" } },
  }));

  part("head", box({
    parent: "body",
    at: [0, -0.03, -0.64],
    size: [0.4, 0.34, 0.36],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.15], axis: [1, 0, 0] },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.08, -0.36],
    size: [0.24, 0.2, 0.46],
    material: "snout",
  }));
  part("snout_tip", box({
    parent: "snout",
    at: [0, -0.015, -0.29],
    size: [0.16, 0.15, 0.16],
    material: "snout",
    faces: { north: { texture: "snout_tip" } },
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.3, -0.25],
    ["fr", 0.3, -0.25],
    ["bl", -0.3, 0.28],
    ["br", 0.3, 0.28],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.31, z],
      size: [0.15, 0.25, 0.16],
      material: "fur_dark",
      joint: { pivot: [0, 0.125, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.16, -0.07],
      size: [0.3, 0.09, 0.32],
      material: "paw",
      faces: { north: { texture: "claw_front" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.01, 0.59],
    rot: [28, 0, 0],
    size: [0.12, 0.15, 0.12],
    material: "spine",
    joint: { pivot: [0, 0.06, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("dig_walk", {
    fps: 18,
    duration: 1.1,
    cycleDistance: 0.48,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.01,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.7,
    swingDegrees: 16,
    tail: "tail",
    tailSwingDegrees: 4,
  });
});
