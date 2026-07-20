import { figure } from "../../src/dsl";

// A box-only North American porcupine with a high stepped quill mantle, small
// dark face, sturdy legs, and a short thick two-stage tail. Alternating pale
// pixels make the quill field read without introducing curved primitives.
export default figure("porcupine", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#4b372b");
  mat("fur_light", "#725540");
  mat("fur_dark", "#2d241e");
  mat("quill", "#514536");
  mat("quill_light", "#b9a77f");
  mat("quill_pale", "#ded0aa");
  mat("belly", "#80634a");
  mat("muzzle", "#8b6c50");
  mat("nose", "#1d1916");
  mat("paw", "#332820");

  asciiTexture("face", {
    palette: { ".": "#4b372b", "l": "#725540", "e": "#171411", "q": "#b9a77f" },
    pixels: [
      "qq....qq",
      ".ee..ee.",
      "..llll..",
      ".llllll.",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#8b6c50", "n": "#1d1916", "l": "#b1916e" },
    pixels: [
      "ll....ll",
      ".lnnnnl.",
      "..nnnn..",
      "........",
      "........",
    ],
  });
  asciiTexture("quill_side", {
    palette: { ".": "#514536", "l": "#b9a77f", "p": "#ded0aa", "d": "#2d241e" },
    pixels: [
      "p..l..p..l..p.",
      ".p..l..p..l..p",
      "..p..l..p..l..",
      "l..p..l..p..l.",
      ".l..p..l..p..l",
      "dd..dd..dd..dd",
    ],
  });
  asciiTexture("quill_top", {
    palette: { ".": "#514536", "l": "#b9a77f", "p": "#ded0aa", "d": "#2d241e" },
    pixels: [
      "p.l.p.l.p.l.p",
      ".p.l.p.l.p.l.",
      "l.p.l.p.l.p.l",
      ".l.p.l.p.l.p.",
      "p.l.p.l.p.l.p",
    ],
  });

  part("body", box({
    at: [0, 0.63, 0.08],
    size: [0.88, 0.5, 1.2],
    material: "fur",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.3, -0.02],
    size: [0.66, 0.12, 0.86],
    material: "belly",
  }));
  part("quill_main", box({
    parent: "body",
    at: [0, 0.25, 0.06],
    size: [0.92, 0.48, 0.72],
    material: "quill",
    faces: {
      east: { texture: "quill_side" },
      west: { texture: "quill_side" },
      up: { texture: "quill_top" },
    },
  }));
  part("quill_front", box({
    parent: "body",
    at: [0, 0.17, -0.4],
    size: [0.84, 0.38, 0.36],
    material: "quill_light",
    faces: {
      east: { texture: "quill_side" },
      west: { texture: "quill_side" },
    },
  }));
  part("quill_rear", box({
    parent: "body",
    at: [0, 0.18, 0.48],
    size: [0.82, 0.4, 0.32],
    material: "quill",
    faces: {
      east: { texture: "quill_side" },
      west: { texture: "quill_side" },
    },
  }));
  part("quill_crown", box({
    parent: "body",
    at: [0, 0.52, 0.05],
    size: [0.7, 0.18, 0.58],
    material: "quill_pale",
    faces: { up: { texture: "quill_top" } },
  }));
  part("quill_shoulder", box({
    parent: "body",
    at: [0, 0.37, -0.32],
    size: [0.72, 0.2, 0.3],
    material: "quill_pale",
    faces: { up: { texture: "quill_top" } },
  }));

  part("head", box({
    parent: "body",
    at: [0, -0.03, -0.75],
    size: [0.5, 0.42, 0.48],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.2], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.34],
    size: [0.34, 0.22, 0.24],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.19], ["r", 0.19]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.24, 0.07],
      size: [0.13, 0.15, 0.1],
      material: "fur_dark",
      joint: { pivot: [0, -0.055, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.3, -0.34],
    ["fr", 0.3, -0.34],
    ["bl", -0.3, 0.38],
    ["br", 0.3, 0.38],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.39, z],
      size: [0.17, 0.34, 0.18],
      material: "fur_dark",
      joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.21, -0.05],
      size: [0.24, 0.1, 0.28],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.01, 0.75],
    rot: [-8, 0, 0],
    size: [0.38, 0.3, 0.5],
    material: "quill",
    faces: { up: { texture: "quill_top" } },
    joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.05, 0.39],
    size: [0.25, 0.2, 0.34],
    material: "quill_light",
    faces: { up: { texture: "quill_top" } },
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.3,
    cycleDistance: 0.58,
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
    bodyBob: 0.013,
    bodyBobCenter: 0.015,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.72,
    swingDegrees: 14,
    tail: "tail",
    tailSwingDegrees: 5,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.4, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.4, lag: 0.12 }),
    ],
  });
});
