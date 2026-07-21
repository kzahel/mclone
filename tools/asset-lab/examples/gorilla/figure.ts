import { figure } from "../../src/dsl";

// A box-only silverback gorilla shaped for knuckle walking. The wide shoulder
// beam and long two-stage arms dominate the compact hips, while a gray back
// saddle and heavy brow distinguish the adult male at sheet scale.
export default figure("gorilla", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#292827");
  mat("fur_light", "#3b3937");
  mat("fur_shadow", "#171716");
  mat("silver", "#777773");
  mat("silver_light", "#92918b");
  mat("skin", "#4d4540");
  mat("skin_light", "#6d5d53");
  mat("eye", "#b58b52");
  mat("nail", "#262220");

  asciiTexture("face", {
    palette: { ".": "#4d4540", "e": "#171411", "a": "#b58b52", "d": "#292827" },
    pixels: [
      "dddddddd",
      "d......d",
      "d.a..a.d",
      "d.e..e.d",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#6d5d53", "n": "#211d1b", "m": "#352825" },
    pixels: [
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "..mmmm..",
      ".mmmmmm.",
      "........",
    ],
  });
  asciiTexture("chest", {
    palette: { ".": "#292827", "l": "#3b3937", "s": "#777773" },
    pixels: [
      "ss......ss",
      ".ss....ss.",
      "..llllll..",
      ".llllllll.",
      "....ll....",
      "..........",
    ],
  });

  part("torso", box({
    at: [0, 1.13, 0.08],
    size: [0.94, 1.08, 0.66],
    material: "fur",
    faces: { north: { texture: "chest" } },
  }));
  part("shoulders", box({
    parent: "torso",
    at: [0, 0.34, -0.02],
    size: [1.38, 0.48, 0.72],
    material: "fur_light",
  }));
  part("silver_back", box({
    parent: "torso",
    at: [0, 0.04, 0.37],
    size: [0.86, 0.82, 0.14],
    material: "silver",
  }));
  part("silver_shoulders", box({
    parent: "shoulders",
    at: [0, 0.02, 0.39],
    size: [1.16, 0.34, 0.12],
    material: "silver_light",
  }));
  part("belly", box({
    parent: "torso",
    at: [0, -0.28, -0.37],
    size: [0.62, 0.42, 0.12],
    material: "fur_light",
  }));

  part("neck", box({
    parent: "torso",
    at: [0, 0.54, -0.16],
    rot: [-10, 0, 0],
    size: [0.62, 0.38, 0.5],
    material: "fur_shadow",
    joint: { pivot: [0, -0.16, 0.12], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.33, -0.17],
    rot: [8, 0, 0],
    size: [0.7, 0.58, 0.54],
    material: "skin",
    faces: { north: { texture: "face" } },
  }));
  part("brow", box({
    parent: "head",
    at: [0, 0.18, -0.3],
    size: [0.58, 0.18, 0.12],
    material: "fur_shadow",
  }));
  part("crown", box({
    parent: "head",
    at: [0, 0.33, 0.02],
    size: [0.5, 0.2, 0.46],
    material: "fur",
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.16, -0.4],
    size: [0.52, 0.34, 0.3],
    material: "skin_light",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.35], ["r", 0.35]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.02, 0.04],
      size: [0.15, 0.2, 0.1],
      material: "skin_light",
    }));
  }

  for (const [side, x] of [["l", -0.56], ["r", 0.56]] as const) {
    part(`upper_arm_${side}`, box({
      parent: "shoulders",
      at: [x, -0.45, -0.08],
      rot: [-4, 0, side === "l" ? -4 : 4],
      size: [0.34, 0.76, 0.38],
      material: "fur",
      joint: { pivot: [0, 0.38, 0], axis: [1, 0, 0] },
    }));
    part(`forearm_${side}`, box({
      parent: `upper_arm_${side}`,
      at: [0, -0.56, -0.1],
      rot: [-5, 0, 0],
      size: [0.38, 0.66, 0.42],
      material: "fur_light",
    }));
    part(`knuckle_${side}`, box({
      parent: `forearm_${side}`,
      at: [0, -0.39, -0.1],
      size: [0.44, 0.2, 0.42],
      material: "skin",
    }));
    for (const [nailIndex, nailX] of [-0.13, 0, 0.13].entries()) {
      part(`nail_${side}_${nailIndex + 1}`, box({
        parent: `knuckle_${side}`,
        at: [nailX, -0.02, -0.222],
        size: [0.075, 0.065, 0.025],
        material: "nail",
      }));
    }
  }

  for (const [side, x] of [["l", -0.28], ["r", 0.28]] as const) {
    part(`leg_${side}`, box({
      parent: "torso",
      at: [x, -0.7, 0.12],
      rot: [-8, 0, 0],
      size: [0.36, 0.56, 0.4],
      material: "fur_shadow",
      joint: { pivot: [0, 0.28, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.34, -0.12],
      size: [0.42, 0.18, 0.54],
      material: "skin",
    }));
  }

  quadrupedWalk("knuckle_walk", {
    fps: 18,
    duration: 1.38,
    cycleDistance: 0.62,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "knuckle_l",
      frontRight: "knuckle_r",
      backLeft: "foot_l",
      backRight: "foot_r",
    },
    body: "torso",
    bodyBob: 0.018,
    bodyBobCenter: 0.019,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "upper_arm_l",
      frontRight: "upper_arm_r",
      backLeft: "leg_l",
      backRight: "leg_r",
    },
    stanceRatio: 0.74,
    swingDegrees: 11,
    tracks: [
      followThrough("neck", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.3, lag: 0.12 }),
    ],
  });
});
