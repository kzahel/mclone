import { figure } from "../../src/dsl";

// A box-only adult chimpanzee with a compact dark torso, pale ears and muzzle,
// long knuckle-bearing arms, and lighter proportions than the gorilla rig.
export default figure("chimpanzee", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#342b26");
  mat("fur_light", "#4b3d34");
  mat("fur_dark", "#1d1a18");
  mat("skin", "#4c403a");
  mat("skin_light", "#9b806e");
  mat("skin_shadow", "#655248");
  mat("eye", "#d0a45b");
  mat("nail", "#24201e");

  asciiTexture("face", {
    palette: { ".": "#4c403a", "m": "#1d1a18", "e": "#d0a45b", "l": "#9b806e" },
    pixels: [
      "mmmmmmmm",
      "m......m",
      "m.e..e.m",
      "m.m..m.m",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#9b806e", "n": "#211c19", "s": "#655248" },
    pixels: [
      "..nnnn..",
      ".nn..nn.",
      "........",
      "..ssss..",
      ".ssssss.",
      "........",
    ],
  });
  asciiTexture("chest", {
    palette: { ".": "#342b26", "l": "#4b3d34", "s": "#655248" },
    pixels: [
      "ll......ll",
      ".ll....ll.",
      "..ssssss..",
      "...ssss...",
      "....ss....",
      "..........",
    ],
  });

  part("torso", box({
    at: [0, 1.02, 0.06],
    size: [0.76, 0.86, 0.58],
    material: "fur",
    faces: { north: { texture: "chest" } },
  }));
  part("shoulders", box({
    parent: "torso",
    at: [0, 0.29, -0.03],
    size: [1.04, 0.34, 0.62],
    material: "fur_light",
  }));
  part("belly", box({
    parent: "torso",
    at: [0, -0.25, -0.32],
    size: [0.52, 0.32, 0.1],
    material: "skin_shadow",
  }));

  part("neck", box({
    parent: "torso",
    at: [0, 0.45, -0.12],
    rot: [-8, 0, 0],
    size: [0.48, 0.3, 0.4],
    material: "fur_dark",
    joint: { pivot: [0, -0.12, 0.1], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.27, -0.15],
    rot: [7, 0, 0],
    size: [0.56, 0.5, 0.46],
    material: "skin",
    faces: { north: { texture: "face" } },
  }));
  part("brow", box({
    parent: "head",
    at: [0, 0.14, -0.26],
    size: [0.5, 0.13, 0.1],
    material: "fur_dark",
  }));
  part("crown", box({
    parent: "head",
    at: [0, 0.28, 0.02],
    size: [0.45, 0.16, 0.38],
    material: "fur",
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.14, -0.33],
    size: [0.4, 0.26, 0.24],
    material: "skin_light",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.3], ["r", 0.3]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.01, 0.02],
      size: [0.14, 0.19, 0.1],
      material: "skin_light",
    }));
  }

  for (const [side, x] of [["l", -0.43], ["r", 0.43]] as const) {
    part(`upper_arm_${side}`, box({
      parent: "shoulders",
      at: [x, -0.4, -0.07],
      rot: [-3, 0, side === "l" ? -3 : 3],
      size: [0.25, 0.64, 0.29],
      material: "fur",
      joint: { pivot: [0, 0.32, 0], axis: [1, 0, 0] },
    }));
    part(`forearm_${side}`, box({
      parent: `upper_arm_${side}`,
      at: [0, -0.47, -0.07],
      rot: [-4, 0, 0],
      size: [0.29, 0.55, 0.32],
      material: "fur_light",
    }));
    part(`knuckle_${side}`, box({
      parent: `forearm_${side}`,
      at: [0, -0.33, -0.08],
      size: [0.32, 0.18, 0.34],
      material: "skin",
    }));
    for (const [nailIndex, nailX] of [-0.085, 0.085].entries()) {
      part(`nail_${side}_${nailIndex + 1}`, box({
        parent: `knuckle_${side}`,
        at: [nailX, -0.015, -0.182],
        size: [0.065, 0.055, 0.02],
        material: "nail",
      }));
    }
  }

  for (const [side, x] of [["l", -0.22], ["r", 0.22]] as const) {
    part(`leg_${side}`, box({
      parent: "torso",
      at: [x, -0.58, 0.12],
      rot: [-7, 0, 0],
      size: [0.28, 0.53, 0.32],
      material: "fur_dark",
      joint: { pivot: [0, 0.265, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.32, -0.1],
      size: [0.32, 0.17, 0.42],
      material: "skin",
    }));
  }

  quadrupedWalk("knuckle_walk", {
    fps: 18,
    duration: 1.05,
    cycleDistance: 0.72,
    gait: "walk",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "knuckle_l",
      frontRight: "knuckle_r",
      backLeft: "foot_l",
      backRight: "foot_r",
    },
    body: "torso",
    bodyBob: 0.015,
    bodyBobCenter: 0.017,
    head: "head",
    headSwingDegrees: 3,
    legs: {
      frontLeft: "upper_arm_l",
      frontRight: "upper_arm_r",
      backLeft: "leg_l",
      backRight: "leg_r",
    },
    stanceRatio: 0.67,
    swingDegrees: 15,
    tracks: [
      followThrough("neck", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.4, lag: 0.1 }),
    ],
  });
});
