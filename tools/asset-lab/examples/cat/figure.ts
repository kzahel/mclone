import { figure } from "../../src/dsl";

// A sparse box-only house cat. The two-piece tail follows the same cuboid rig
// vocabulary as the vanilla ocelot; tabby detail is painted instead of modeled.
export default figure("cat", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#646b76");
  mat("fur_dark", "#303743");
  mat("fur_light", "#b9bec5");
  mat("paw", "#252932");

  asciiTexture("face", {
    palette: {
      ".": "#646b76",
      "s": "#303743",
      "e": "#8bd05b",
      "w": "#f2f4f8",
      "n": "#d08a9b",
    },
    pixels: [
      "s......s",
      ".ee..ee.",
      ".ee..ee.",
      "...ss...",
      "..wwww..",
      "...nn...",
      "........",
      "........",
    ],
  });

  asciiTexture("tabby_side", {
    palette: {
      ".": "#646b76",
      "s": "#303743",
      "l": "#b9bec5",
    },
    pixels: [
      "..s..s..s...",
      ".ss..s..ss..",
      "............",
      "............",
      "..llllllll..",
      ".llllllllll.",
    ],
  });

  asciiTexture("tail_rings", {
    palette: {
      ".": "#646b76",
      "s": "#303743",
    },
    pixels: [
      "ssss",
      "....",
      "....",
      "ssss",
      "....",
      "ssss",
    ],
  });

  part("body", box({
    at: [0, 0.75, 0],
    size: [0.76, 0.46, 1.04],
    material: "fur",
    faces: {
      east: { texture: "tabby_side" },
      west: { texture: "tabby_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.24, 0.04],
    size: [0.58, 0.1, 0.76],
    material: "fur_light",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.06, -0.7],
    size: [0.5, 0.42, 0.44],
    material: "fur",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.2], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.29],
    size: [0.3, 0.15, 0.14],
    material: "fur_light",
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.18, 0.27, -0.02],
    rot: [0, 0, -12],
    size: [0.14, 0.22, 0.1],
    material: "fur_dark",
    joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.18, 0.27, -0.02],
    rot: [0, 0, 12],
    size: [0.14, 0.22, 0.1],
    material: "fur_dark",
    joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] },
  }));

  for (const [suffix, x, z, dark] of [
    ["fl", -0.26, -0.31, false],
    ["fr", 0.26, -0.31, false],
    ["bl", -0.26, 0.34, true],
    ["br", 0.26, 0.34, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.42, z],
      size: [0.13, 0.42, 0.14],
      material: dark ? "fur_dark" : "fur",
      joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.25, -0.035],
      size: [0.17, 0.09, 0.21],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.36, 0.52],
    rot: [58, 0, 0],
    size: [0.12, 0.54, 0.12],
    material: "fur_dark",
    joint: { pivot: [0, -0.27, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0.38, 0.12],
    rot: [24, 0, 0],
    size: [0.1, 0.38, 0.1],
    material: "fur",
    faces: {
      north: { texture: "tail_rings" },
      south: { texture: "tail_rings" },
      east: { texture: "tail_rings" },
      west: { texture: "tail_rings" },
    },
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 0.82,
    cycleDistance: 0.78,
    gait: "walk",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.009,
    bodyBobCenter: 0.011,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.62,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 10,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 15, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 15, overshoot: 0.6, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 11, overshoot: 0.75, lag: 0.18 }),
    ],
  });
});
