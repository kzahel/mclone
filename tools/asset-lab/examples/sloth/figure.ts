import { figure } from "../../src/dsl";

// A box-only brown-throated sloth in a grounded crawl, with a masked face,
// low horizontal body, exceptionally long two-stage forelimbs, bent hind
// limbs, and broad hooked contact paws. Its gait is deliberately very slow.
export default figure("sloth", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#756550");
  mat("fur_light", "#9b886d");
  mat("fur_dark", "#493f35");
  mat("cream", "#d0c19f");
  mat("mask", "#51483d");
  mat("nose", "#211e1a");
  mat("paw", "#403930");
  mat("claw", "#d8c9a7");

  asciiTexture("face", {
    palette: { ".": "#d0c19f", "m": "#51483d", "e": "#171512", "f": "#756550" },
    pixels: [
      "ff....ff",
      "fmm..mmf",
      ".me..em.",
      ".mm..mm.",
      "...ff...",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#d0c19f", "n": "#211e1a", "m": "#51483d" },
    pixels: [
      "mm....mm",
      ".mnnnnm.",
      "..nnnn..",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#756550", "l": "#9b886d", "d": "#493f35", "c": "#d0c19f" },
    pixels: [
      "dddddddddddd",
      "dlllllllllld",
      "l..........l",
      "............",
      "..cccccccc..",
      "...cccccc...",
    ],
  });
  asciiTexture("hooked_claws", {
    palette: { ".": "#403930", "c": "#d8c9a7" },
    pixels: [
      "c..c..c.",
      "cc.cc.cc",
      "........",
    ],
  });

  part("body", box({
    at: [0, 0.86, 0.08],
    size: [0.62, 0.48, 1.08],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.29, -0.02],
    size: [0.48, 0.11, 0.76],
    material: "fur_light",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.07, -0.43],
    size: [0.68, 0.5, 0.4],
    material: "fur_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.04, 0.43],
    size: [0.66, 0.48, 0.42],
    material: "fur_dark",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.08, -0.72],
    size: [0.58, 0.5, 0.48],
    material: "cream",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.2], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.14, -0.33],
    size: [0.38, 0.22, 0.2],
    material: "cream",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.25], ["r", 0.25]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.21, 0.05],
      size: [0.12, 0.14, 0.09],
      material: "fur_dark",
      joint: { pivot: [0, -0.05, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [side, x] of [["l", -0.25], ["r", 0.25]] as const) {
    part(`upper_arm_${side}`, box({
      parent: "shoulders",
      at: [x, -0.28, -0.12],
      rot: [-7, 0, side === "l" ? 4 : -4],
      size: [0.17, 0.48, 0.18],
      material: "fur",
      joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] },
    }));
    part(`forearm_${side}`, box({
      parent: `upper_arm_${side}`,
      at: [0, -0.34, -0.08],
      rot: [10, 0, side === "l" ? -3 : 3],
      size: [0.16, 0.44, 0.17],
      material: "fur_light",
      joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
    }));
    part(`hand_${side}`, box({
      parent: `forearm_${side}`,
      at: [0, -0.27, -0.1],
      size: [0.28, 0.12, 0.38],
      material: "paw",
      faces: { north: { texture: "hooked_claws" } },
    }));
  }

  for (const [side, x] of [["l", -0.23], ["r", 0.23]] as const) {
    part(`upper_leg_${side}`, box({
      parent: "rump",
      at: [x, -0.32, 0.03],
      rot: [6, 0, side === "l" ? -4 : 4],
      size: [0.19, 0.4, 0.21],
      material: "fur_dark",
      joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] },
    }));
    part(`lower_leg_${side}`, box({
      parent: `upper_leg_${side}`,
      at: [0, -0.29, -0.03],
      rot: [-8, 0, 0],
      size: [0.17, 0.34, 0.18],
      material: "fur",
      joint: { pivot: [0, 0.16, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `lower_leg_${side}`,
      at: [0, -0.23, -0.1],
      size: [0.28, 0.13, 0.38],
      material: "paw",
      faces: { north: { texture: "hooked_claws" } },
    }));
  }

  quadrupedWalk("crawl", {
    fps: 18,
    duration: 2.4,
    cycleDistance: 0.42,
    gait: "walk",
    loop: true,
    samples: 25,
    contactParts: {
      frontLeft: "hand_l",
      frontRight: "hand_r",
      backLeft: "foot_l",
      backRight: "foot_r",
    },
    body: "body",
    bodyBob: 0.008,
    bodyBobCenter: 0.01,
    head: "head",
    headSwingDegrees: 1.5,
    legs: {
      frontLeft: "upper_arm_l",
      frontRight: "upper_arm_r",
      backLeft: "upper_leg_l",
      backRight: "upper_leg_r",
    },
    stanceRatio: 0.8,
    swingDegrees: 10,
    tracks: [
      followThrough("forearm_l", { source: "upper_arm_l", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 5, overshoot: 0.25, lag: 0.1 }),
      followThrough("forearm_r", { source: "upper_arm_r", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 5, overshoot: 0.25, lag: 0.1 }),
      followThrough("lower_leg_l", { source: "upper_leg_l", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 4, overshoot: 0.2, lag: 0.1 }),
      followThrough("lower_leg_r", { source: "upper_leg_r", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 4, overshoot: 0.2, lag: 0.1 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.4, lag: 0.15 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.4, lag: 0.15 }),
    ],
  });
});
