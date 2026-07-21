import { figure } from "../../src/dsl";

// A box-only Indian grey mongoose with a narrow salt-and-pepper body, pointed
// two-stage face, low quick legs, and a long three-stage balancing tail. Its
// rapid trot stays close to the ground while the tail follows laterally.
export default figure("mongoose", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  swing,
  followThrough,
}) => {
  mat("fur", "#817764");
  mat("fur_light", "#a69a83");
  mat("fur_dark", "#4d493f");
  mat("cream", "#c9bda4");
  mat("muzzle", "#a3947d");
  mat("nose", "#211f1b");
  mat("paw", "#3b3932");

  asciiTexture("face", {
    palette: { ".": "#817764", "l": "#a69a83", "e": "#171613", "d": "#4d493f" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      "..llll..",
      ".llllll.",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#a3947d", "n": "#211f1b", "c": "#c9bda4" },
    pixels: [
      "cc....cc",
      ".cnnnnc.",
      "..nnnn..",
      "........",
    ],
  });
  asciiTexture("speckled_side", {
    palette: { ".": "#817764", "l": "#a69a83", "d": "#4d493f", "c": "#c9bda4" },
    pixels: [
      "d.ld.ld.ld.ld.",
      ".d.ld.ld.ld.ld",
      "l.dl.dl.dl.dl.",
      "..............",
      "..cccccccccc..",
      "...cccccccc...",
    ],
  });
  asciiTexture("paw_front", {
    palette: { ".": "#3b3932", "c": "#1f1e1b" },
    pixels: [
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.55, 0.07],
    size: [0.56, 0.38, 1.28],
    material: "fur",
    faces: {
      east: { texture: "speckled_side" },
      west: { texture: "speckled_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.24, -0.02],
    size: [0.42, 0.1, 0.92],
    material: "cream",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.03, -0.52],
    size: [0.6, 0.38, 0.36],
    material: "fur_light",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.06, -0.75],
    size: [0.42, 0.38, 0.42],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.17], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.3],
    size: [0.28, 0.2, 0.22],
    material: "muzzle",
  }));
  part("muzzle_tip", box({
    parent: "muzzle",
    at: [0, -0.025, -0.16],
    size: [0.18, 0.14, 0.12],
    material: "nose",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.17], ["r", 0.17]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.23, 0.06],
      size: [0.12, 0.14, 0.09],
      material: "fur_dark",
      joint: { pivot: [0, -0.05, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.19, -0.38],
    ["fr", 0.19, -0.38],
    ["bl", -0.19, 0.4],
    ["br", 0.19, 0.4],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.31, z],
      size: [0.12, 0.3, 0.14],
      material: "fur_dark",
      joint: { pivot: [0, 0.15, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.19, -0.045],
      size: [0.19, 0.09, 0.25],
      material: "paw",
      faces: { north: { texture: "paw_front" } },
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.04, 0.82],
    size: [0.3, 0.25, 0.58],
    material: "fur",
    joint: { pivot: [0, 0, -0.27], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.02, 0.49],
    size: [0.22, 0.18, 0.52],
    material: "fur_light",
    joint: { pivot: [0, 0, -0.24], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, -0.02, 0.4],
    size: [0.14, 0.12, 0.38],
    material: "fur_dark",
  }));

  quadrupedWalk("dart", {
    fps: 24,
    duration: 0.68,
    cycleDistance: 0.78,
    gait: "trot",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 3,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.58,
    swingDegrees: 21,
    tracks: [
      swing("tail_1", { axis: "y", degrees: 9, phase: 0.5 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 13, overshoot: 0.5, lag: 0.11 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.5, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.5, lag: 0.1 }),
    ],
  });
});
