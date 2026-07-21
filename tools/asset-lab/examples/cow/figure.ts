import { figure } from "../../src/dsl";

// A box-only Holstein using texture-painted patches and the compact vanilla
// cow silhouette: one broad body, a low head, tiny horns, and four stout legs.
export default figure("cow", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("hide", "#f2f1ee");
  mat("hide_shadow", "#d8d6d1");
  mat("spot", "#1d1b19");
  mat("muzzle", "#caa6a8");
  mat("ear_inner", "#b58a8c");
  mat("horn", "#d9cfb6");
  mat("hoof", "#26201d");
  mat("udder", "#e7b1b0");

  asciiTexture("face", {
    palette: { ".": "#f2f1ee", "s": "#1d1b19", "e": "#16110f" },
    pixels: [
      "ss....ss",
      "see..ees",
      ".ee..ee.",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#caa6a8", "n": "#3a2c2d" },
    pixels: [
      "........",
      "........",
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "........",
    ],
  });
  asciiTexture("hide_side", {
    palette: { ".": "#f2f1ee", "s": "#1d1b19" },
    pixels: [
      "....ssss....",
      "...ssssss...",
      "ss.sss......",
      "ssss........",
      ".ss.....ssss",
      "........ssss",
      "..........ss",
      "............",
    ],
  });
  asciiTexture("leg_sock", {
    palette: { ".": "#f2f1ee", "s": "#1d1b19" },
    pixels: [
      "....",
      "....",
      "....",
      "s..s",
      "ssss",
      "ssss",
    ],
  });

  part("body", box({
    at: [0, 1.08, 0],
    size: [1, 0.82, 1.58],
    material: "hide",
    faces: {
      east: { texture: "hide_side" },
      west: { texture: "hide_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.43, 0.18],
    size: [0.86, 0.1, 1.04],
    material: "hide_shadow",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.12, -0.84],
    rot: [16, 0, 0],
    size: [0.62, 0.58, 0.5],
    material: "spot",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.12, -0.43],
    size: [0.7, 0.56, 0.52],
    material: "hide",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.23], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.18, -0.36],
    size: [0.5, 0.27, 0.24],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.39, 0.09, 0.03],
    rot: [0, 0, -18],
    size: [0.3, 0.15, 0.14],
    material: "ear_inner",
    joint: { pivot: [0.13, 0, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.39, 0.09, 0.03],
    rot: [0, 0, 18],
    size: [0.3, 0.15, 0.14],
    material: "ear_inner",
    joint: { pivot: [-0.13, 0, 0], axis: [1, 0, 0] },
  }));

  for (const [side, x, tilt] of [["l", -0.19, -22], ["r", 0.19, 22]] as const) {
    part(`horn_${side}`, box({
      parent: "head",
      at: [x, 0.35, -0.02],
      rot: [0, 0, tilt],
      size: [0.1, 0.22, 0.1],
      material: "horn",
    }));
    part(`horn_${side}_tip`, box({
      parent: `horn_${side}`,
      at: [0, 0.15, 0],
      size: [0.065, 0.13, 0.065],
      material: "horn",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.34, -0.5],
    ["fr", 0.34, -0.5],
    ["bl", -0.34, 0.54],
    ["br", 0.34, 0.54],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.65, z],
      size: [0.22, 0.58, 0.23],
      material: "hide",
      faces: {
        north: { texture: "leg_sock" },
        south: { texture: "leg_sock" },
        east: { texture: "leg_sock" },
        west: { texture: "leg_sock" },
      },
      joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.34, -0.025],
      size: [0.25, 0.12, 0.28],
      material: "hoof",
    }));
  }

  part("udder", box({
    parent: "body",
    at: [0, -0.5, 0.43],
    size: [0.34, 0.22, 0.34],
    material: "udder",
  }));
  part("tail", box({
    parent: "body",
    at: [0, -0.02, 0.86],
    rot: [-8, 0, 0],
    size: [0.09, 0.68, 0.09],
    material: "hide_shadow",
    joint: { pivot: [0, 0.34, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.41, 0.02],
    size: [0.18, 0.2, 0.17],
    material: "spot",
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.1,
    cycleDistance: 0.82,
    gait: "walk",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.66,
    swingDegrees: 15,
    tail: "tail",
    tailSwingDegrees: 9,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.6, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.7, lag: 0.18 }),
    ],
  });
});
