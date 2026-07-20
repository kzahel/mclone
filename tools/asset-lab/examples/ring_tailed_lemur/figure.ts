import { figure } from "../../src/dsl";

// A box-only ring-tailed lemur with a narrow gray body, white ruff, masked
// amber-eyed face, and a long upright tail built from alternating cuboid bands.
export default figure("ring_tailed_lemur", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#7b7a76");
  mat("fur_light", "#a8a59d");
  mat("fur_dark", "#3b3b3a");
  mat("black", "#171817");
  mat("white", "#dedbd1");
  mat("cream", "#c8c2b4");
  mat("eye", "#d89b2b");
  mat("paw", "#292a29");

  asciiTexture("face", {
    palette: { ".": "#dedbd1", "m": "#171817", "e": "#d89b2b", "g": "#7b7a76" },
    pixels: [
      "gg....gg",
      "gmm..mmg",
      ".me..em.",
      ".mm..mm.",
      "...mm...",
      "...mm...",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#dedbd1", "n": "#171817" },
    pixels: [
      "........",
      "..nnnn..",
      ".nnnnnn.",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#7b7a76", "l": "#a8a59d", "d": "#3b3b3a" },
    pixels: [
      "dddddddddddd",
      "d..........d",
      "..llllllll..",
      ".llllllllll.",
      "............",
      "..dddddddd..",
    ],
  });

  part("body", box({
    at: [0, 0.73, 0.04],
    size: [0.54, 0.43, 1.08],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.25, -0.02],
    size: [0.42, 0.1, 0.78],
    material: "fur_light",
  }));
  part("ruff", box({
    parent: "body",
    at: [0, 0.07, -0.55],
    size: [0.58, 0.45, 0.2],
    material: "white",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.12, -0.72],
    size: [0.46, 0.45, 0.42],
    material: "white",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.18], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.13, -0.29],
    size: [0.27, 0.21, 0.2],
    material: "white",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.2], ["r", 0.2]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.25, 0.02],
      rot: [0, 0, side === "l" ? -12 : 12],
      size: [0.14, 0.21, 0.1],
      material: "fur_light",
      joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z, shade] of [
    ["fl", -0.18, -0.32, "fur_light"],
    ["fr", 0.18, -0.32, "fur_light"],
    ["bl", -0.18, 0.34, "fur_dark"],
    ["br", 0.18, 0.34, "fur_dark"],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.39, z],
      size: [0.11, 0.39, 0.12],
      material: shade,
      joint: { pivot: [0, 0.195, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.23, -0.035],
      size: [0.15, 0.09, 0.2],
      material: "paw",
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.27, 0.56],
    rot: [43, 0, 0],
    size: [0.2, 0.48, 0.2],
    material: "black",
    joint: { pivot: [0, -0.23, 0], axis: [0, 0, 1] },
  }));
  for (const [index, material, width] of [
    [2, "white", 0.2],
    [3, "black", 0.2],
    [4, "white", 0.19],
    [5, "black", 0.18],
    [6, "white", 0.17],
    [7, "black", 0.15],
  ] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, 0.33, index < 5 ? 0.055 : 0.025],
      rot: [index < 4 ? 7 : -4, 0, 0],
      size: [width, index === 7 ? 0.28 : 0.34, width],
      material,
    }));
  }

  quadrupedWalk("walk", {
    fps: 18,
    duration: 0.9,
    cycleDistance: 0.82,
    gait: "walk",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 3,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.62,
    swingDegrees: 20,
    tail: "tail_1",
    tailSwingDegrees: 9,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.55, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.55, lag: 0.11 }),
      followThrough("tail_1", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.7, lag: 0.16 }),
    ],
  });
});
