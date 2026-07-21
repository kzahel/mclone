import { figure } from "../../src/dsl";

// A box-only white-faced capuchin with a compact dark body, pale face and
// shoulders, long grasping limbs, and a seven-stage prehensile tail held in a
// loose forward curl.
export default figure("capuchin_monkey", ({
  asciiTexture,
  box,
  followThrough,
  mat,
  part,
  quadrupedWalk,
}) => {
  mat("fur", "#382f28");
  mat("fur_light", "#57483b");
  mat("fur_dark", "#201c19");
  mat("cream", "#d8c7a7");
  mat("cream_shadow", "#aa9679");
  mat("skin", "#a88970");
  mat("eye", "#161513");
  mat("eye_glint", "#e5dfc8");
  mat("nail", "#2b2521");

  asciiTexture("face", {
    palette: { ".": "#d8c7a7", "s": "#a88970", "e": "#161513", "h": "#e5dfc8", "d": "#201c19" },
    pixels: [
      "dddddddddd",
      "d........d",
      "..ee..ee..",
      "..eh..he..",
      "...ssss...",
      "..s....s..",
      "...ssss...",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#a88970", "n": "#201c19", "l": "#d8c7a7" },
    pixels: [
      "..nnnn..",
      ".nn..nn.",
      "........",
      "..llll..",
      ".llllll.",
    ],
  });
  asciiTexture("back_mottle", {
    palette: { ".": "#382f28", "l": "#57483b", "d": "#201c19" },
    pixels: [
      "dddddddddd",
      "dll....lld",
      "d..l..l..d",
      "dl......ld",
      "d..llll..d",
      "dddddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.82, 0.05],
    rot: [-5, 0, 0],
    size: [0.58, 0.58, 0.86],
    material: "fur",
    faces: { up: { texture: "back_mottle" } },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.2, -0.34],
    size: [0.42, 0.32, 0.12],
    material: "fur_light",
  }));
  part("shoulder_mantle", box({
    parent: "body",
    at: [0, 0.13, -0.34],
    size: [0.68, 0.38, 0.3],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.25, -0.51],
    rot: [-12, 0, 0],
    size: [0.4, 0.34, 0.34],
    material: "cream_shadow",
    joint: { pivot: [0, -0.14, 0.12], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.25, -0.17],
    rot: [8, 0, 0],
    size: [0.5, 0.48, 0.44],
    material: "cream",
    faces: { north: { texture: "face" } },
  }));
  part("cap", box({
    parent: "head",
    at: [0, 0.25, 0.02],
    size: [0.48, 0.18, 0.38],
    material: "fur_dark",
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.13, -0.29],
    size: [0.34, 0.22, 0.2],
    material: "skin",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.27, 0, 0.02],
      size: [0.12, 0.18, 0.1],
      material: "skin",
    }));

    part(`upper_arm_${side}`, box({
      parent: "shoulder_mantle",
      at: [sign * 0.34, -0.28, -0.04],
      rot: [-4, 0, sign * -4],
      size: [0.17, 0.48, 0.2],
      material: "cream_shadow",
      joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] },
    }));
    part(`forearm_${side}`, box({
      parent: `upper_arm_${side}`,
      at: [0, -0.37, -0.04],
      rot: [-5, 0, 0],
      size: [0.18, 0.4, 0.21],
      material: "fur_dark",
    }));
    part(`hand_${side}`, box({
      parent: `forearm_${side}`,
      at: [0, -0.24, -0.07],
      size: [0.22, 0.11, 0.28],
      material: "skin",
    }));

    part(`thigh_${side}`, box({
      parent: "body",
      at: [sign * 0.21, -0.39, 0.3],
      rot: [-8, 0, 0],
      size: [0.25, 0.4, 0.3],
      material: "fur",
      joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${side}`, box({
      parent: `thigh_${side}`,
      at: [0, -0.3, -0.04],
      rot: [7, 0, 0],
      size: [0.18, 0.3, 0.2],
      material: "fur_dark",
    }));
    part(`foot_${side}`, box({
      parent: `shin_${side}`,
      at: [0, -0.2, -0.1],
      size: [0.22, 0.1, 0.34],
      material: "skin",
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.12, 0.55],
    rot: [52, 0, 0],
    size: [0.18, 0.5, 0.18],
    material: "fur_dark",
    joint: { pivot: [0, -0.23, 0], axis: [1, 0, 0] },
  }));
  for (const [index, rotation, width] of [
    [2, 17, 0.175],
    [3, 18, 0.17],
    [4, 20, 0.16],
    [5, 24, 0.145],
    [6, 28, 0.13],
    [7, 32, 0.11],
  ] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, 0.34, 0.035],
      rot: [rotation, 0, 0],
      size: [width, index === 7 ? 0.26 : 0.36, width],
      material: index >= 6 ? "fur_light" : "fur_dark",
      joint: { pivot: [0, -0.16, 0], axis: [1, 0, 0] },
    }));
  }

  quadrupedWalk("hand_walk", {
    fps: 20,
    duration: 0.92,
    cycleDistance: 0.7,
    gait: "walk",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "hand_l",
      frontRight: "hand_r",
      backLeft: "foot_l",
      backRight: "foot_r",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "neck",
    headSwingDegrees: 3,
    legs: {
      frontLeft: "upper_arm_l",
      frontRight: "upper_arm_r",
      backLeft: "thigh_l",
      backRight: "thigh_r",
    },
    stanceRatio: 0.64,
    swingDegrees: 18,
    tail: "tail_1",
    tailSwingDegrees: 7,
    tracks: [
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 8, overshoot: 0.45, lag: 0.09 }),
      followThrough("tail_4", { source: "tail_2", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 10, overshoot: 0.5, lag: 0.1 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.4, lag: 0.08 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.4, lag: 0.08 }),
    ],
  });
});
