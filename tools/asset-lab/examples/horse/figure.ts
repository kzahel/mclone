import { figure } from "../../src/dsl";

// A box-only bay horse based on the vanilla long barrel, angled neck, narrow
// head, mane ridge, and tall legs. Lower-leg color is painted onto the legs.
export default figure("horse", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#8a5a2b");
  mat("coat_shadow", "#724824");
  mat("muzzle", "#5b3a1c");
  mat("point", "#1a140f");
  mat("hoof", "#171210");
  mat("ear_inner", "#6e4a26");

  asciiTexture("face", {
    palette: { ".": "#8a5a2b", "e": "#120c08", "w": "#e9e3d6" },
    pixels: [
      "...ww...",
      "ee.ww.ee",
      "ee.ww.ee",
      "...ww...",
      "...ww...",
      "..www...",
      "..www...",
      "..www...",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#5b3a1c", "n": "#2a1a0d" },
    pixels: [
      "........",
      "........",
      "........",
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("leg_point", {
    palette: { ".": "#8a5a2b", "p": "#1a140f" },
    pixels: [
      "....",
      "....",
      "....",
      "....",
      "pppp",
      "pppp",
      "pppp",
      "pppp",
    ],
  });

  part("body", box({
    at: [0, 1.18, 0],
    size: [0.78, 0.66, 1.76],
    material: "coat",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.35, 0.04],
    size: [0.64, 0.1, 1.26],
    material: "coat_shadow",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.39, -0.83],
    rot: [-42, 0, 0],
    size: [0.42, 0.84, 0.46],
    material: "coat",
    joint: { pivot: [0, -0.42, 0], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.5, -0.18],
    rot: [30, 0, 0],
    size: [0.36, 0.42, 0.54],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.43],
    size: [0.28, 0.28, 0.35],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.12, 0.28, 0.12],
    rot: [-6, 0, -9],
    size: [0.11, 0.24, 0.09],
    material: "ear_inner",
    joint: { pivot: [0, -0.1, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.12, 0.28, 0.12],
    rot: [-6, 0, 9],
    size: [0.11, 0.24, 0.09],
    material: "ear_inner",
    joint: { pivot: [0, -0.1, 0], axis: [1, 0, 0] },
  }));

  for (const [index, y, size] of [
    [1, 0.31, [0.12, 0.2, 0.14]],
    [2, 0.13, [0.13, 0.22, 0.14]],
    [3, -0.06, [0.14, 0.24, 0.14]],
    [4, -0.25, [0.14, 0.24, 0.14]],
  ] as const) {
    part(`mane_${index}`, box({
      parent: "neck",
      at: [0, y, 0.24],
      rot: [6, 0, 0],
      size,
      material: "point",
    }));
  }
  part("forelock", box({
    parent: "head",
    at: [0, 0.19, 0.19],
    rot: [22, 0, 0],
    size: [0.13, 0.16, 0.11],
    material: "point",
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.27, -0.58],
    ["fr", 0.27, -0.58],
    ["bl", -0.27, 0.62],
    ["br", 0.27, 0.62],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.58, z],
      size: [0.18, 0.74, 0.19],
      material: "coat",
      faces: {
        north: { texture: "leg_point" },
        south: { texture: "leg_point" },
        east: { texture: "leg_point" },
        west: { texture: "leg_point" },
      },
      joint: { pivot: [0, 0.37, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.43, -0.01],
      size: [0.21, 0.12, 0.24],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.2, 0.9],
    rot: [20, 0, 0],
    size: [0.16, 0.66, 0.18],
    material: "point",
    joint: { pivot: [0, 0.33, 0], axis: [0, 0, 1] },
  }));
  part("tail_skirt", box({
    parent: "tail",
    at: [0, -0.52, -0.04],
    rot: [-12, 0, 0],
    size: [0.24, 0.52, 0.25],
    material: "point",
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
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.8, lag: 0.2 }),
    ],
  });
});
