import { figure } from "../../src/dsl";

// A box-only adult male orangutan. Rust-orange shag, dark cheek flanges, a
// compact body, and exceptionally long forelimbs carry its readable silhouette.
export default figure("orangutan", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#a84e20");
  mat("fur_light", "#c66a2b");
  mat("fur_dark", "#733118");
  mat("fur_shadow", "#4d2518");
  mat("skin", "#3f3832");
  mat("skin_light", "#736157");
  mat("skin_shadow", "#272421");
  mat("eye", "#c39a54");
  mat("nail", "#1f1c1a");

  asciiTexture("face", {
    palette: { ".": "#3f3832", "d": "#272421", "e": "#c39a54", "l": "#736157" },
    pixels: [
      "dddddddd",
      "d......d",
      "d.e..e.d",
      "d.d..d.d",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#736157", "n": "#211f1d", "m": "#4a3c36" },
    pixels: [
      "..nnnn..",
      ".nn..nn.",
      "........",
      "..mmmm..",
      ".mmmmmm.",
      "........",
    ],
  });
  asciiTexture("shag", {
    palette: { ".": "#a84e20", "l": "#c66a2b", "d": "#733118" },
    pixels: [
      "ll......ll",
      ".ll....ll.",
      "..dddddd..",
      ".d......d.",
      "....ll....",
      "...llll...",
    ],
  });

  part("torso", box({
    at: [0, 1.0, 0.08],
    size: [0.84, 0.84, 0.62],
    material: "fur",
    faces: { north: { texture: "shag" } },
  }));
  part("shoulders", box({
    parent: "torso",
    at: [0, 0.31, -0.02],
    size: [1.12, 0.38, 0.68],
    material: "fur_light",
  }));
  part("belly", box({
    parent: "torso",
    at: [0, -0.25, -0.35],
    size: [0.58, 0.32, 0.1],
    material: "fur_dark",
  }));
  for (const [side, x] of [["l", -0.47], ["r", 0.47]] as const) {
    part(`shoulder_shag_${side}`, box({
      parent: "shoulders",
      at: [x, -0.16, 0.05],
      size: [0.22, 0.54, 0.58],
      material: "fur_dark",
    }));
  }

  part("neck", box({
    parent: "torso",
    at: [0, 0.43, -0.13],
    rot: [-11, 0, 0],
    size: [0.54, 0.3, 0.43],
    material: "fur_shadow",
    joint: { pivot: [0, -0.12, 0.1], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.26, -0.17],
    rot: [8, 0, 0],
    size: [0.5, 0.5, 0.46],
    material: "skin",
    faces: { north: { texture: "face" } },
  }));
  part("crown", box({
    parent: "head",
    at: [0, 0.28, 0.02],
    size: [0.48, 0.18, 0.4],
    material: "fur_dark",
  }));
  part("brow", box({
    parent: "head",
    at: [0, 0.14, -0.27],
    size: [0.46, 0.13, 0.1],
    material: "skin_shadow",
  }));
  for (const [side, x] of [["l", -0.34], ["r", 0.34]] as const) {
    part(`cheek_${side}`, box({
      parent: "head",
      at: [x, -0.01, -0.02],
      size: [0.2, 0.46, 0.4],
      material: "skin_shadow",
    }));
  }
  part("muzzle", box({
    parent: "head",
    at: [0, -0.14, -0.34],
    size: [0.38, 0.27, 0.25],
    material: "skin_light",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("chin", box({
    parent: "muzzle",
    at: [0, -0.18, 0.04],
    size: [0.32, 0.13, 0.2],
    material: "fur_shadow",
  }));

  for (const [side, x] of [["l", -0.5], ["r", 0.5]] as const) {
    part(`upper_arm_${side}`, box({
      parent: "shoulders",
      at: [x, -0.42, -0.04],
      rot: [-3, 0, side === "l" ? -4 : 4],
      size: [0.28, 0.79, 0.33],
      material: "fur",
      joint: { pivot: [0, 0.395, 0], axis: [1, 0, 0] },
    }));
    part(`forearm_${side}`, box({
      parent: `upper_arm_${side}`,
      at: [0, -0.48, -0.08],
      rot: [-4, 0, 0],
      size: [0.32, 0.63, 0.37],
      material: "fur_light",
    }));
    part(`knuckle_${side}`, box({
      parent: `forearm_${side}`,
      at: [0, -0.36, -0.08],
      size: [0.36, 0.18, 0.38],
      material: "skin",
    }));
    for (const [nailIndex, nailX] of [-0.1, 0.1].entries()) {
      part(`nail_${side}_${nailIndex + 1}`, box({
        parent: `knuckle_${side}`,
        at: [nailX, -0.015, -0.202],
        size: [0.07, 0.055, 0.02],
        material: "nail",
      }));
    }
  }

  for (const [side, x] of [["l", -0.23], ["r", 0.23]] as const) {
    part(`leg_${side}`, box({
      parent: "torso",
      at: [x, -0.59, 0.13],
      rot: [-9, 0, 0],
      size: [0.3, 0.47, 0.34],
      material: "fur_dark",
      joint: { pivot: [0, 0.235, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.28, -0.11],
      size: [0.35, 0.17, 0.46],
      material: "skin",
    }));
  }

  quadrupedWalk("long_arm_walk", {
    fps: 18,
    duration: 1.52,
    cycleDistance: 0.58,
    gait: "walk",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "knuckle_l",
      frontRight: "knuckle_r",
      backLeft: "foot_l",
      backRight: "foot_r",
    },
    body: "torso",
    bodyBob: 0.02,
    bodyBobCenter: 0.022,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "upper_arm_l",
      frontRight: "upper_arm_r",
      backLeft: "leg_l",
      backRight: "leg_r",
    },
    stanceRatio: 0.78,
    swingDegrees: 10,
    tracks: [
      followThrough("neck", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.14 }),
      followThrough("shoulder_shag_l", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "z", degrees: 2.5, overshoot: 0.55, lag: 0.16 }),
      followThrough("shoulder_shag_r", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "z", degrees: 2.5, overshoot: 0.55, lag: 0.16 }),
    ],
  });
});
