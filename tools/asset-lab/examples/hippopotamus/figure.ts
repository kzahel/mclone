import { figure } from "../../src/dsl";

// A box-only hippopotamus with a low barrel body, tiny ears, high-set eyes,
// and an intentionally oversized squared muzzle. Short planted legs make its
// walk read as weight transfer rather than a brisk quadruped stride.
export default figure("hippopotamus", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("hide", "#776c70");
  mat("hide_light", "#958489");
  mat("hide_shadow", "#5d5357");
  mat("muzzle", "#a58b8d");
  mat("muzzle_light", "#bc9a98");
  mat("ear_inner", "#b27779");
  mat("eye", "#171315");
  mat("mouth", "#4c3137");
  mat("toenail", "#d0b8ad");

  asciiTexture("muzzle_face", {
    palette: { ".": "#a58b8d", "n": "#48363a", "l": "#bc9a98" },
    pixels: [
      "........",
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "ll....ll",
      "llllllll",
    ],
  });
  asciiTexture("side_folds", {
    palette: { ".": "#776c70", "d": "#5d5357", "l": "#958489" },
    pixels: [
      "............",
      "..lll.......",
      "......dd....",
      "....dd..d...",
      "...d.....d..",
      "....dd..d...",
      "......dd....",
      "............",
    ],
  });

  part("body", box({
    at: [0, 0.94, 0.12],
    size: [1.38, 0.82, 1.58],
    material: "hide",
    faces: {
      east: { texture: "side_folds" },
      west: { texture: "side_folds" },
    },
  }));
  part("back", box({
    parent: "body",
    at: [0, 0.39, 0.16],
    size: [1.22, 0.14, 1.3],
    material: "hide_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.43, 0.08],
    size: [1.18, 0.12, 1.26],
    material: "hide_shadow",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.02, 0.67],
    size: [1.3, 0.68, 0.48],
    material: "hide_light",
  }));

  part("head", box({
    parent: "body",
    at: [0, 0.02, -0.89],
    rot: [-2, 0, 0],
    size: [1.02, 0.72, 0.66],
    material: "hide_light",
    joint: { pivot: [0, 0, 0.27], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.16, -0.48],
    size: [1.12, 0.48, 0.42],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("lower_jaw", box({
    parent: "muzzle",
    at: [0, -0.29, 0.035],
    size: [0.94, 0.13, 0.34],
    material: "mouth",
  }));
  for (const [side, x] of [["l", -0.34], ["r", 0.34]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [x, 0.4, -0.13],
      size: [0.24, 0.18, 0.25],
      material: "hide",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0.015, -0.135],
      size: [0.08, 0.08, 0.035],
      material: "eye",
    }));
    part(`ear_${side}`, box({
      parent: "head",
      at: [x * 1.35, 0.4, 0.17],
      rot: [0, 0, side === "l" ? -14 : 14],
      size: [0.2, 0.23, 0.12],
      material: "ear_inner",
      joint: { pivot: [side === "l" ? 0.06 : -0.06, -0.07, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.48, -0.48],
    ["fr", 0.48, -0.48],
    ["bl", -0.47, 0.5],
    ["br", 0.47, 0.5],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.58, z],
      size: [0.3, 0.42, 0.32],
      material: "hide",
      joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.27, -0.055],
      size: [0.42, 0.15, 0.44],
      material: "hide_shadow",
    }));
    for (const [toeIndex, toeX] of [-0.105, 0.105].entries()) {
      part(`toe_${suffix}_${toeIndex + 1}`, box({
        parent: `foot_${suffix}`,
        at: [toeX, -0.012, -0.232],
        size: [0.09, 0.065, 0.025],
        material: "toenail",
      }));
    }
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.04, 0.9],
    rot: [-18, 0, 0],
    size: [0.1, 0.34, 0.1],
    material: "hide_shadow",
    joint: { pivot: [0, 0.17, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.22, 0.02],
    size: [0.16, 0.13, 0.14],
    material: "hide_shadow",
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.46,
    cycleDistance: 0.7,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.019,
    bodyBobCenter: 0.02,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.74,
    swingDegrees: 12,
    tail: "tail",
    tailSwingDegrees: 6,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.4, lag: 0.13 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.4, lag: 0.13 }),
    ],
  });
});
