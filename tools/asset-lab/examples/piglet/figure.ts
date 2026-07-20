import { figure } from "../../src/dsl";

// A compact box-only piglet following the vanilla pig's large square head and
// projecting snout. The small raised tail remains a single animated cuboid.
export default figure("piglet", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("skin", "#d88a92");
  mat("skin_dark", "#bd6f7b");
  mat("hoof", "#4a3033");

  asciiTexture("head_face", {
    palette: { ".": "#d88a92", "e": "#261718" },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#bd6f7b", "n": "#261718" },
    pixels: [
      "........",
      "........",
      "..n..n..",
      "..n..n..",
      "........",
      "........",
    ],
  });

  part("body", box({
    at: [0, 0.84, 0],
    size: [1.14, 0.68, 0.82],
    material: "skin",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.15, -0.65],
    size: [0.72, 0.62, 0.62],
    material: "skin",
    faces: { north: { texture: "head_face" } },
    joint: { pivot: [0, 0, 0.28], axis: [1, 0, 0] },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.08, -0.39],
    size: [0.38, 0.22, 0.18],
    material: "skin_dark",
    faces: { north: { texture: "snout_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.27, 0.37, -0.05],
    rot: [0, 0, -9],
    size: [0.17, 0.23, 0.1],
    material: "skin",
    joint: { pivot: [0, -0.1, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.27, 0.37, -0.05],
    rot: [0, 0, 9],
    size: [0.17, 0.23, 0.1],
    material: "skin",
    joint: { pivot: [0, -0.1, 0], axis: [1, 0, 0] },
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.4, -0.25],
    ["fr", 0.4, -0.25],
    ["bl", -0.4, 0.27],
    ["br", 0.4, 0.27],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.51, z],
      size: [0.18, 0.42, 0.18],
      material: "skin",
      joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.27, -0.015],
      size: [0.2, 0.1, 0.21],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.22, 0.46],
    rot: [55, 0, 0],
    size: [0.13, 0.25, 0.13],
    material: "skin_dark",
    joint: { pivot: [0, -0.11, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1,
    cycleDistance: 0.72,
    gait: "trot",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.0125,
    bodyBobCenter: 0.0125,
    head: "head",
    headSwingDegrees: 4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.56,
    swingDegrees: 20,
    tail: "tail",
    tailSwingDegrees: 15,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 17, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 17, overshoot: 0.6, lag: 0.12 }),
    ],
  });
});
