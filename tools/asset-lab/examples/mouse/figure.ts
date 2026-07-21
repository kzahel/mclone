import { figure } from "../../src/dsl";

// A box-only field mouse with a compact mottled body, pointed muzzle, large
// stepped ears, six whiskers, tiny planted paws, and a four-part pink tail.
export default figure("mouse", ({
  asciiTexture,
  box,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
  swing,
}) => {
  mat("fur", "#8a6d52");
  mat("fur_light", "#aa8a69");
  mat("fur_dark", "#5e493a");
  mat("belly", "#d6c5a7");
  mat("ear", "#a86f73");
  mat("ear_inner", "#d29496");
  mat("eye", "#171410");
  mat("nose", "#6e3f45");
  mat("paw", "#c39188");
  mat("tail", "#b77977");
  mat("tail_light", "#ce908c");
  mat("whisker", "#e8ddc8");

  asciiTexture("fur_mottle", {
    palette: { ".": "#8a6d52", "l": "#aa8a69", "d": "#5e493a" },
    pixels: [
      "llllllllll",
      "l..d.....l",
      "l.d..d.d.l",
      "l....d...l",
      "l.d....d.l",
      "llllllllll",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#d6c5a7", "m": "#8a5c58", "d": "#5e493a" },
    pixels: [
      "........",
      "..d..d..",
      ".m....m.",
      "...mm...",
    ],
  });
  asciiTexture("paw_toes", {
    palette: { ".": "#c39188", "d": "#6e3f45" },
    pixels: [
      ".d.d.d.",
      "ddddddd",
      ".......",
    ],
  });

  part("body", box({
    at: [0, 0.42, 0.08],
    rot: [-4, 0, 0],
    size: [0.5, 0.38, 0.84],
    material: "fur",
    faces: {
      up: { texture: "fur_mottle" },
      east: { texture: "fur_mottle" },
      west: { texture: "fur_mottle" },
    },
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.03, 0.3],
    size: [0.58, 0.42, 0.5],
    material: "fur_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.23, -0.08],
    size: [0.4, 0.12, 0.52],
    material: "belly",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.03, -0.52],
    size: [0.42, 0.36, 0.4],
    material: "fur_light",
    joint: { pivot: [0, 0, 0.17], axis: [0, 1, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.08, -0.3],
    size: [0.28, 0.2, 0.28],
    material: "belly",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("nose", box({
    parent: "muzzle",
    at: [0, 0.01, -0.18],
    size: [0.14, 0.12, 0.1],
    material: "nose",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.17, 0.28, 0.04],
      rot: [0, 0, sign * -8],
      size: [0.22, 0.32, 0.1],
      material: "ear",
      joint: { pivot: [0, -0.14, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0, -0.07],
      size: [0.12, 0.22, 0.05],
      material: "ear_inner",
    }));
    part(`eye_${side}`, box({
      parent: "head",
      at: [sign * 0.14, 0.05, -0.22],
      size: [0.09, 0.09, 0.055],
      material: "eye",
    }));

    for (const [index, y, roll] of [[1, 0.04, 9], [2, -0.01, 0], [3, -0.06, -9]] as const) {
      part(`whisker_${side}_${index}`, box({
        parent: "muzzle",
        at: [sign * 0.2, y, -0.17],
        rot: [0, sign * -5, sign * roll],
        size: [0.34, 0.018, 0.018],
        material: "whisker",
      }));
    }
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.18, -0.24, false],
    ["fr", 0.18, -0.24, false],
    ["bl", -0.2, 0.27, true],
    ["br", 0.2, 0.27, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.24, z],
      size: rear ? [0.18, 0.26, 0.18] : [0.14, 0.24, 0.14],
      material: rear ? "fur_dark" : "fur",
      joint: { pivot: [0, rear ? 0.12 : 0.11, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.14, -0.07],
      size: rear ? [0.2, 0.08, 0.28] : [0.18, 0.08, 0.24],
      material: "paw",
      faces: { up: { texture: "paw_toes" } },
    }));
  }

  part("tail_1", box({
    parent: "rump",
    at: [0, -0.08, 0.48],
    rot: [-14, 0, 0],
    size: [0.12, 0.12, 0.58],
    material: "tail",
    joint: { pivot: [0, 0, -0.27], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, 0.02, 0.49],
    rot: [10, 0, 0],
    size: [0.1, 0.1, 0.5],
    material: "tail_light",
    joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] },
  }));
  part("tail_3", box({
    parent: "tail_2",
    at: [0, -0.01, 0.43],
    rot: [8, 0, 0],
    size: [0.08, 0.08, 0.46],
    material: "tail",
    joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_3",
    at: [0, -0.015, 0.36],
    rot: [10, 0, 0],
    size: [0.055, 0.055, 0.34],
    material: "tail_light",
  }));

  quadrupedWalk("scurry", {
    label: "Quick scurry",
    role: "locomotion",
    fps: 24,
    duration: 0.48,
    cycleDistance: 0.42,
    gait: "trot",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.014,
    bodyBobPhase: 0.5,
    head: "head",
    headSwingDegrees: 2.4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.58,
    swingDegrees: 23,
    tracks: [
      swing("tail_1", { axis: "y", degrees: 9, phase: 0.05 }),
      swing("tail_2", { axis: "y", degrees: 13, phase: 0.17 }),
      swing("tail_3", { axis: "y", degrees: 16, phase: 0.29 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.1 }),
    ],
  });
  defaultClip("scurry");
});
