import { figure } from "../../src/dsl";

// A box-only plains zebra that reuses the reviewed horse proportions while
// making its identity almost entirely texture-driven: vertical body/neck
// stripes, barred legs, a dark muzzle, upright mane, and tufted tail.
export default figure("zebra", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("white", "#f2eadb");
  mat("white_shadow", "#d8d0c4");
  mat("black", "#171717");
  mat("muzzle", "#625b5a");
  mat("hoof", "#161616");
  mat("ear_inner", "#b9857f");

  asciiTexture("face", {
    palette: { ".": "#f2eadb", "s": "#171717", "e": "#8d6f38" },
    pixels: [
      "ss....ss",
      ".se..es.",
      "..s..s..",
      ".s....s.",
      "s..ss..s",
      "..s..s..",
      ".s....s.",
      "s......s",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#625b5a", "n": "#111111" },
    pixels: [
      "........",
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#f2eadb", "s": "#171717" },
    pixels: [
      "ss...ss..ss...ss",
      ".ss..s....s..ss.",
      "..s..ss..ss..s..",
      "..ss..s..s..ss..",
      ".ss...s..s...ss.",
      "ss....s..s....ss",
      "s.....s..s.....s",
      "......s..s......",
    ],
  });
  asciiTexture("body_top", {
    palette: { ".": "#f2eadb", "s": "#171717" },
    pixels: [
      "s..s..s..s",
      ".s..ss..s.",
      "..s....s..",
      ".s..ss..s.",
      "s..s..s..s",
    ],
  });
  asciiTexture("neck_side", {
    palette: { ".": "#f2eadb", "s": "#171717" },
    pixels: [
      "s...s...",
      ".s...s..",
      "..s...s.",
      "...s...s",
      "s...s...",
      ".s...s..",
      "..s...s.",
      "...s...s",
      "s...s...",
      ".s...s..",
    ],
  });
  asciiTexture("leg_stripes", {
    palette: { ".": "#f2eadb", "s": "#171717" },
    pixels: [
      "ssss",
      "....",
      "....",
      "ssss",
      "....",
      "ssss",
      "....",
      "ssss",
    ],
  });

  part("body", box({
    at: [0, 1.18, 0],
    size: [0.8, 0.68, 1.74],
    material: "white",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
      up: { texture: "body_top" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.36, 0.04],
    size: [0.65, 0.1, 1.24],
    material: "white_shadow",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.4, -0.82],
    rot: [-42, 0, 0],
    size: [0.44, 0.84, 0.48],
    material: "white",
    faces: {
      east: { texture: "neck_side" },
      west: { texture: "neck_side" },
    },
    joint: { pivot: [0, -0.42, 0], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.5, -0.18],
    rot: [30, 0, 0],
    size: [0.38, 0.43, 0.55],
    material: "white",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.4],
    size: [0.28, 0.24, 0.28],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.13, 0.29, 0.12],
    rot: [-6, 0, -9],
    size: [0.12, 0.25, 0.09],
    material: "ear_inner",
    joint: { pivot: [0, -0.11, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.13, 0.29, 0.12],
    rot: [-6, 0, 9],
    size: [0.12, 0.25, 0.09],
    material: "ear_inner",
    joint: { pivot: [0, -0.11, 0], axis: [1, 0, 0] },
  }));

  for (const [index, y, size] of [
    [1, 0.31, [0.13, 0.21, 0.14]],
    [2, 0.13, [0.14, 0.23, 0.14]],
    [3, -0.06, [0.15, 0.25, 0.14]],
    [4, -0.25, [0.15, 0.25, 0.14]],
  ] as const) {
    part(`mane_${index}`, box({
      parent: "neck",
      at: [0, y, 0.25],
      rot: [6, 0, 0],
      size,
      material: "black",
    }));
  }
  part("forelock", box({
    parent: "head",
    at: [0, 0.2, 0.2],
    rot: [22, 0, 0],
    size: [0.14, 0.17, 0.11],
    material: "black",
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.27, -0.58],
    ["fr", 0.27, -0.58],
    ["bl", -0.27, 0.62],
    ["br", 0.27, 0.62],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.59, z],
      size: [0.19, 0.75, 0.2],
      material: "white",
      faces: {
        north: { texture: "leg_stripes" },
        south: { texture: "leg_stripes" },
        east: { texture: "leg_stripes" },
        west: { texture: "leg_stripes" },
      },
      joint: { pivot: [0, 0.375, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.435, -0.01],
      size: [0.22, 0.12, 0.25],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.2, 0.89],
    rot: [20, 0, 0],
    size: [0.16, 0.6, 0.18],
    material: "black",
    joint: { pivot: [0, 0.3, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.45, -0.04],
    rot: [-12, 0, 0],
    size: [0.24, 0.38, 0.25],
    material: "black",
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1,
    cycleDistance: 1.05,
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
    stanceRatio: 0.64,
    swingDegrees: 19,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.12 }),
      followThrough("mane_1", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.14 }),
      followThrough("mane_2", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.7, lag: 0.16 }),
      followThrough("mane_3", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.75, lag: 0.18 }),
      followThrough("mane_4", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 11, overshoot: 0.75, lag: 0.2 }),
    ],
  });
});
