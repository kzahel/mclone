import { figure } from "../../src/dsl";

// A box-only adult raccoon with a broad mask, pale muzzle, dark paws, compact
// torso, and a four-stage ringed tail. Its cautious walk favors long planted
// phases and restrained body motion.
export default figure("raccoon", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#777b78");
  mat("fur_light", "#a9aaa3");
  mat("fur_dark", "#343837");
  mat("fur_shadow", "#555956");
  mat("cream", "#d3cbb8");
  mat("black", "#171918");
  mat("paw", "#282b2a");

  asciiTexture("face", {
    palette: { ".": "#777b78", "m": "#343837", "e": "#171918", "l": "#a9aaa3" },
    pixels: [
      "ll....ll",
      "mmmmmmmm",
      "mme..emm",
      "mme..emm",
      ".mmmmmm.",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#d3cbb8", "n": "#171918" },
    pixels: [
      "........",
      "..nnnn..",
      ".nnnnnn.",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#777b78", "d": "#555956", "l": "#a9aaa3" },
    pixels: [
      "dddddddddddd",
      "d..........d",
      "..llllllll..",
      ".l........l.",
      "............",
      "..dddddddd..",
    ],
  });

  part("body", box({
    at: [0, 0.7, 0.06],
    size: [0.74, 0.52, 1.18],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.3, 0.02],
    size: [0.56, 0.12, 0.84],
    material: "fur_light",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.08, -0.73],
    size: [0.58, 0.46, 0.5],
    material: "fur",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.22], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.34],
    size: [0.34, 0.22, 0.22],
    material: "cream",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.21], ["r", 0.21]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.29, 0.01],
      rot: [0, 0, side === "l" ? -10 : 10],
      size: [0.17, 0.23, 0.12],
      material: "fur_dark",
      joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.25, -0.35],
    ["fr", 0.25, -0.35],
    ["bl", -0.25, 0.37],
    ["br", 0.25, 0.37],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.42, z],
      size: [0.15, 0.4, 0.16],
      material: suffix[0] === "b" ? "fur_shadow" : "fur_dark",
      joint: { pivot: [0, 0.2, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.24, -0.045],
      size: [0.21, 0.09, 0.26],
      material: "paw",
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.28, 0.66],
    rot: [48, 0, 0],
    size: [0.3, 0.58, 0.3],
    material: "fur_dark",
    joint: { pivot: [0, -0.28, 0], axis: [0, 0, 1] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, 0.39, 0.09],
    rot: [8, 0, 0],
    size: [0.32, 0.38, 0.32],
    material: "fur_light",
  }));
  part("tail_3", box({
    parent: "tail_2",
    at: [0, 0.29, 0.04],
    rot: [7, 0, 0],
    size: [0.28, 0.3, 0.28],
    material: "fur_dark",
  }));
  part("tail_tip", box({
    parent: "tail_3",
    at: [0, 0.23, 0.02],
    size: [0.22, 0.22, 0.22],
    material: "fur_light",
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.08,
    cycleDistance: 0.64,
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
    bodyBob: 0.01,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.7,
    swingDegrees: 16,
    tail: "tail_1",
    tailSwingDegrees: 8,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.12 }),
      followThrough("tail_1", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.65, lag: 0.16 }),
    ],
  });
});
