import { figure } from "../../src/dsl";

// A box-only adult red panda with a rust coat, white facial mask and ruff,
// dark legs, and a plush six-section ringed tail held above the back.
export default figure("red_panda", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#a94d2d");
  mat("fur_light", "#ca7044");
  mat("fur_dark", "#432d27");
  mat("fur_shadow", "#713923");
  mat("cream", "#eadbc2");
  mat("white", "#f1e7d3");
  mat("black", "#22211f");
  mat("paw", "#2b2522");

  asciiTexture("face", {
    palette: { ".": "#f1e7d3", "r": "#a94d2d", "m": "#432d27", "e": "#171513" },
    pixels: [
      "rr....rr",
      "rmm..mmr",
      ".me..em.",
      ".mm..mm.",
      "...rr...",
      "..rrrr..",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#f1e7d3", "n": "#22211f", "c": "#eadbc2" },
    pixels: [
      "..cccc..",
      ".cccccc.",
      "..nnnn..",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#a94d2d", "l": "#ca7044", "d": "#713923", "c": "#eadbc2" },
    pixels: [
      "dddddddddddd",
      "d..........d",
      "..llllllll..",
      ".l........l.",
      "..cccccccc..",
      "...cccccc...",
    ],
  });

  part("body", box({
    at: [0, 0.72, 0.05],
    size: [0.68, 0.5, 1.08],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.28, 0],
    size: [0.5, 0.11, 0.76],
    material: "fur_dark",
  }));
  part("ruff", box({
    parent: "body",
    at: [0, 0.08, -0.54],
    size: [0.7, 0.5, 0.18],
    material: "cream",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.14, -0.7],
    size: [0.54, 0.48, 0.48],
    material: "white",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.21], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.14, -0.33],
    size: [0.32, 0.21, 0.21],
    material: "white",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.21], ["r", 0.21]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.3, 0.02],
      rot: [0, 0, side === "l" ? -9 : 9],
      size: [0.17, 0.23, 0.12],
      material: "fur_dark",
      joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.22, -0.32],
    ["fr", 0.22, -0.32],
    ["bl", -0.23, 0.34],
    ["br", 0.23, 0.34],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.4, z],
      size: [0.14, 0.38, 0.15],
      material: "fur_dark",
      joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.23, -0.045],
      size: [0.2, 0.09, 0.25],
      material: "paw",
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.27, 0.58],
    rot: [47, 0, 0],
    size: [0.28, 0.52, 0.28],
    material: "fur_shadow",
    joint: { pivot: [0, -0.25, 0], axis: [0, 0, 1] },
  }));
  for (const [index, material, width, height, tilt] of [
    [2, "cream", 0.3, 0.38, 8],
    [3, "fur", 0.29, 0.36, 7],
    [4, "cream", 0.27, 0.34, 5],
    [5, "fur_shadow", 0.24, 0.31, -3],
    [6, "cream", 0.2, 0.26, -6],
  ] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, index === 2 ? 0.38 : 0.3, 0.04],
      rot: [tilt, 0, 0],
      size: [width, height, width],
      material,
    }));
  }

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.02,
    cycleDistance: 0.65,
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
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.67,
    swingDegrees: 17,
    tail: "tail_1",
    tailSwingDegrees: 9,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.11 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 13, overshoot: 0.8, lag: 0.17 }),
    ],
  });
});
