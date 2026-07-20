import { legacyFigure } from "../../src/dsl";

export default legacyFigure("bear_rounded", ({ mat, asciiTexture, part, box, capsule, sphere, quadrupedWalk, followThrough }) => {
  mat("fur", "#6f4328");
  mat("fur_mid", "#7c5030");
  mat("fur_dark", "#3f2819");
  mat("fur_shadow", "#2c1c13");
  mat("muzzle", "#a97a52");
  mat("ear_inner", "#8b5638");
  mat("eye", "#120d09");
  mat("nose", "#17100c");
  mat("claw", "#15100c");

  asciiTexture("face", {
    palette: {
      ".": "#6f4328",
      "d": "#3f2819",
      "e": "#120d09",
      "m": "#a97a52",
    },
    pixels: [
      "dddddddd",
      "dd....dd",
      ".e....e.",
      "..mmmm..",
      "..mmmm..",
      "...mm...",
      "........",
      "........",
    ],
  });

  asciiTexture("snout_face", {
    palette: {
      ".": "#a97a52",
      "d": "#7c5030",
      "n": "#17100c",
    },
    pixels: [
      "..dddd..",
      ".dddddd.",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
      "........",
      "........",
    ],
  });

  part("body", box({ size: [1.02, 0.64, 1.58], material: "fur" }));
  part("shoulder_hump", box({ parent: "body", at: [0, 0.42, -0.44], rot: [-4, 0, 0], size: [0.92, 0.32, 0.62], material: "fur_dark" }));
  part("chest", box({ parent: "body", at: [0, -0.02, -0.62], size: [1.08, 0.58, 0.38], material: "fur_mid" }));
  part("belly", box({ parent: "body", at: [0, -0.34, -0.02], size: [0.84, 0.12, 1.16], material: "fur_shadow" }));
  part("rump", box({ parent: "body", at: [0, 0.04, 0.58], size: [0.96, 0.56, 0.48], material: "fur_mid" }));
  part("haunch_l", box({ parent: "body", at: [-0.42, -0.02, 0.5], size: [0.22, 0.48, 0.42], material: "fur_dark" }));
  part("haunch_r", box({ parent: "body", at: [0.42, -0.02, 0.5], size: [0.22, 0.48, 0.42], material: "fur_dark" }));
  part("shoulder_l", box({ parent: "body", at: [-0.42, -0.04, -0.52], size: [0.24, 0.52, 0.42], material: "fur_dark" }));
  part("shoulder_r", box({ parent: "body", at: [0.42, -0.04, -0.52], size: [0.24, 0.52, 0.42], material: "fur_dark" }));

  part("neck", box({
    parent: "body",
    at: [0, 0.2, -0.84],
    rot: [-10, 0, 0],
    size: [0.62, 0.44, 0.42],
    material: "fur_mid",
    joint: { pivot: [0, -0.22, 0.14], axis: [1, 0, 0] },
  }));

  part("head", box({
    parent: "neck",
    at: [0, 0.12, -0.4],
    rot: [8, 0, 0],
    size: [0.66, 0.5, 0.52],
    material: "fur",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("brow", box({ parent: "head", at: [0, 0.12, -0.23], size: [0.68, 0.12, 0.16], material: "fur_dark" }));
  part("cheek_l", box({ parent: "head", at: [-0.24, -0.08, -0.2], size: [0.16, 0.18, 0.18], material: "muzzle" }));
  part("cheek_r", box({ parent: "head", at: [0.24, -0.08, -0.2], size: [0.16, 0.18, 0.18], material: "muzzle" }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.37],
    size: [0.44, 0.23, 0.26],
    material: "muzzle",
    faces: {
      north: { texture: "snout_face" },
    },
  }));
  part("nose", sphere({ parent: "muzzle", at: [0, 0.03, -0.15], radius: 0.055, material: "nose" }));

  part("ear_l", sphere({ parent: "head", at: [-0.25, 0.25, -0.02], radius: 0.115, material: "fur_dark" }));
  part("ear_r", sphere({ parent: "head", at: [0.25, 0.25, -0.02], radius: 0.115, material: "fur_dark" }));
  part("ear_l_inner", box({ parent: "ear_l", at: [0, -0.02, -0.08], size: [0.11, 0.09, 0.03], material: "ear_inner" }));
  part("ear_r_inner", box({ parent: "ear_r", at: [0, -0.02, -0.08], size: [0.11, 0.09, 0.03], material: "ear_inner" }));

  part("leg_fl", capsule({ parent: "body", at: [-0.34, -0.43, -0.5], radius: 0.13, length: 0.5, material: "fur_dark", joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.34, -0.43, -0.5], radius: 0.13, length: 0.5, material: "fur_dark", joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.36, -0.43, 0.5], radius: 0.14, length: 0.52, material: "fur_dark", joint: { pivot: [0, 0.26, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.36, -0.43, 0.5], radius: 0.14, length: 0.52, material: "fur_dark", joint: { pivot: [0, 0.26, 0], axis: [1, 0, 0] } }));

  part("paw_fl", box({ parent: "leg_fl", at: [0, -0.32, -0.08], size: [0.3, 0.1, 0.36], material: "fur_shadow" }));
  part("paw_fr", box({ parent: "leg_fr", at: [0, -0.32, -0.08], size: [0.3, 0.1, 0.36], material: "fur_shadow" }));
  part("paw_bl", box({ parent: "leg_bl", at: [0, -0.33, 0.02], size: [0.32, 0.11, 0.38], material: "fur_shadow" }));
  part("paw_br", box({ parent: "leg_br", at: [0, -0.33, 0.02], size: [0.32, 0.11, 0.38], material: "fur_shadow" }));

  part("claw_fl_l", box({ parent: "paw_fl", at: [-0.09, -0.01, -0.21], size: [0.055, 0.035, 0.11], material: "claw" }));
  part("claw_fl_m", box({ parent: "paw_fl", at: [0, -0.01, -0.22], size: [0.055, 0.035, 0.12], material: "claw" }));
  part("claw_fl_r", box({ parent: "paw_fl", at: [0.09, -0.01, -0.21], size: [0.055, 0.035, 0.11], material: "claw" }));
  part("claw_fr_l", box({ parent: "paw_fr", at: [-0.09, -0.01, -0.21], size: [0.055, 0.035, 0.11], material: "claw" }));
  part("claw_fr_m", box({ parent: "paw_fr", at: [0, -0.01, -0.22], size: [0.055, 0.035, 0.12], material: "claw" }));
  part("claw_fr_r", box({ parent: "paw_fr", at: [0.09, -0.01, -0.21], size: [0.055, 0.035, 0.11], material: "claw" }));
  part("claw_bl_l", box({ parent: "paw_bl", at: [-0.1, -0.01, -0.22], size: [0.055, 0.035, 0.11], material: "claw" }));
  part("claw_bl_m", box({ parent: "paw_bl", at: [0, -0.01, -0.23], size: [0.055, 0.035, 0.12], material: "claw" }));
  part("claw_bl_r", box({ parent: "paw_bl", at: [0.1, -0.01, -0.22], size: [0.055, 0.035, 0.11], material: "claw" }));
  part("claw_br_l", box({ parent: "paw_br", at: [-0.1, -0.01, -0.22], size: [0.055, 0.035, 0.11], material: "claw" }));
  part("claw_br_m", box({ parent: "paw_br", at: [0, -0.01, -0.23], size: [0.055, 0.035, 0.12], material: "claw" }));
  part("claw_br_r", box({ parent: "paw_br", at: [0.1, -0.01, -0.22], size: [0.055, 0.035, 0.11], material: "claw" }));

  part("tail", capsule({
    parent: "body",
    at: [0, 0.08, 0.86],
    rot: [38, 0, 0],
    radius: 0.065,
    length: 0.16,
    material: "fur_dark",
    joint: { pivot: [0, 0.08, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.35,
    cycleDistance: 0.76,
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
    bodyBobCenter: 0.01,
    head: "head",
    headSwingDegrees: 1.8,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.72,
    swingDegrees: 13,
    tail: "tail",
    tailSwingDegrees: 3,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.16 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.12 }),
    ],
  });
});
