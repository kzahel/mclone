import { legacyFigure } from "../../src/dsl";

export default legacyFigure("tiger_rounded", ({ mat, asciiTexture, part, box, capsule, sphere, cylinder, quadrupedWalk, followThrough }) => {
  mat("coat", "#e8751a");
  mat("coat_light", "#f09a36");
  mat("coat_shadow", "#b84d13");
  mat("stripe", "#15110e");
  mat("white", "#fff3dc");
  mat("cream", "#f4d8a7");
  mat("ear_inner", "#d37a5f");
  mat("eye", "#f0c44d");
  mat("nose", "#16100d");
  mat("paw", "#3a2215");
  mat("claw", "#f7ead4");
  mat("whisker", "#f8f0de");

  asciiTexture("face", {
    palette: {
      ".": "#e8751a",
      "s": "#15110e",
      "e": "#f0c44d",
      "w": "#fff3dc",
    },
    pixels: [
      "ss....ss",
      "s.s..s.s",
      ".se..es.",
      "..s..s..",
      ".wwssww.",
      ".wwwwww.",
      "..wwww..",
      "........",
    ],
  });

  asciiTexture("snout_face", {
    palette: {
      ".": "#fff3dc",
      "s": "#15110e",
      "n": "#16100d",
      "c": "#f4d8a7",
    },
    pixels: [
      "..cccc..",
      ".cccccc.",
      ".s....s.",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
      "........",
    ],
  });

  asciiTexture("body_side", {
    palette: {
      ".": "#e8751a",
      "o": "#f09a36",
      "s": "#15110e",
      "w": "#fff3dc",
    },
    pixels: [
      "..s..s..s..s",
      ".ss..s..ss..",
      ".s...ss..s..",
      "oo...oo...oo",
      "oww..ww..wwo",
      "wwwwwwwwwwww",
    ],
  });

  asciiTexture("body_top", {
    palette: {
      ".": "#e8751a",
      "s": "#15110e",
      "o": "#f09a36",
    },
    pixels: [
      "s..s..s..s",
      ".s..ss..s.",
      "..oooooo..",
      ".s..ss..s.",
      "s..s..s..s",
    ],
  });

  part("body", box({
    size: [1.06, 0.56, 1.7],
    material: "coat",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
      up: { texture: "body_top" },
    },
  }));
  part("back", box({ parent: "body", at: [0, 0.23, 0.06], size: [0.96, 0.12, 1.18], material: "coat_light" }));
  part("chest", box({ parent: "body", at: [0, -0.02, -0.67], size: [0.82, 0.48, 0.36], material: "white" }));
  part("belly", box({ parent: "body", at: [0, -0.3, -0.02], size: [0.74, 0.12, 1.1], material: "white" }));
  part("haunch_l", box({ parent: "body", at: [-0.43, -0.02, 0.48], size: [0.22, 0.42, 0.46], material: "coat_shadow" }));
  part("haunch_r", box({ parent: "body", at: [0.43, -0.02, 0.48], size: [0.22, 0.42, 0.46], material: "coat_shadow" }));
  part("shoulder_l", box({ parent: "body", at: [-0.43, -0.02, -0.5], size: [0.24, 0.46, 0.42], material: "coat_shadow" }));
  part("shoulder_r", box({ parent: "body", at: [0.43, -0.02, -0.5], size: [0.24, 0.46, 0.42], material: "coat_shadow" }));

  part("stripe_back_1", box({ parent: "body", at: [0, 0.31, -0.48], size: [0.92, 0.035, 0.055], material: "stripe" }));
  part("stripe_back_2", box({ parent: "body", at: [0, 0.31, -0.1], size: [0.86, 0.035, 0.06], material: "stripe" }));
  part("stripe_back_3", box({ parent: "body", at: [0, 0.31, 0.3], size: [0.92, 0.035, 0.055], material: "stripe" }));
  part("stripe_back_4", box({ parent: "body", at: [0, 0.29, 0.66], size: [0.76, 0.035, 0.055], material: "stripe" }));

  part("stripe_l_1", box({ parent: "body", at: [-0.55, 0.07, -0.6], rot: [16, 0, 0], size: [0.035, 0.42, 0.08], material: "stripe" }));
  part("stripe_l_2", box({ parent: "body", at: [-0.55, 0.09, -0.26], rot: [-10, 0, 0], size: [0.035, 0.38, 0.08], material: "stripe" }));
  part("stripe_l_3", box({ parent: "body", at: [-0.55, 0.08, 0.12], rot: [14, 0, 0], size: [0.035, 0.42, 0.08], material: "stripe" }));
  part("stripe_l_4", box({ parent: "body", at: [-0.55, 0.06, 0.46], rot: [-12, 0, 0], size: [0.035, 0.36, 0.08], material: "stripe" }));
  part("stripe_r_1", box({ parent: "body", at: [0.55, 0.07, -0.6], rot: [-16, 0, 0], size: [0.035, 0.42, 0.08], material: "stripe" }));
  part("stripe_r_2", box({ parent: "body", at: [0.55, 0.09, -0.26], rot: [10, 0, 0], size: [0.035, 0.38, 0.08], material: "stripe" }));
  part("stripe_r_3", box({ parent: "body", at: [0.55, 0.08, 0.12], rot: [-14, 0, 0], size: [0.035, 0.42, 0.08], material: "stripe" }));
  part("stripe_r_4", box({ parent: "body", at: [0.55, 0.06, 0.46], rot: [12, 0, 0], size: [0.035, 0.36, 0.08], material: "stripe" }));

  part("neck", box({
    parent: "body",
    at: [0, 0.2, -0.9],
    rot: [-13, 0, 0],
    size: [0.62, 0.46, 0.46],
    material: "coat",
    joint: { pivot: [0, -0.22, 0.16], axis: [1, 0, 0] },
  }));
  part("throat", box({ parent: "neck", at: [0, -0.18, -0.12], size: [0.48, 0.24, 0.3], material: "white" }));
  part("neck_stripe_l", box({ parent: "neck", at: [-0.32, 0.08, -0.04], rot: [14, 0, 0], size: [0.035, 0.32, 0.08], material: "stripe" }));
  part("neck_stripe_r", box({ parent: "neck", at: [0.32, 0.08, -0.04], rot: [-14, 0, 0], size: [0.035, 0.32, 0.08], material: "stripe" }));

  part("head", box({
    parent: "neck",
    at: [0, 0.14, -0.42],
    rot: [8, 0, 0],
    size: [0.6, 0.46, 0.5],
    material: "coat",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("head_top_stripe", box({ parent: "head", at: [0, 0.25, -0.08], size: [0.12, 0.035, 0.34], material: "stripe" }));
  part("brow_l", box({ parent: "head", at: [-0.18, 0.1, -0.24], rot: [0, 0, -10], size: [0.22, 0.08, 0.12], material: "stripe" }));
  part("brow_r", box({ parent: "head", at: [0.18, 0.1, -0.24], rot: [0, 0, 10], size: [0.22, 0.08, 0.12], material: "stripe" }));
  part("cheek_l", box({ parent: "head", at: [-0.24, -0.08, -0.22], size: [0.16, 0.2, 0.18], material: "white" }));
  part("cheek_r", box({ parent: "head", at: [0.24, -0.08, -0.22], size: [0.16, 0.2, 0.18], material: "white" }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.4],
    size: [0.44, 0.22, 0.3],
    material: "white",
    faces: {
      north: { texture: "snout_face" },
    },
  }));
  part("nose", sphere({ parent: "muzzle", at: [0, 0.02, -0.17], radius: 0.052, material: "nose" }));

  part("ear_l", sphere({ parent: "head", at: [-0.28, 0.25, -0.02], radius: 0.11, material: "stripe" }));
  part("ear_r", sphere({ parent: "head", at: [0.28, 0.25, -0.02], radius: 0.11, material: "stripe" }));
  part("ear_l_inner", box({ parent: "ear_l", at: [0, -0.02, -0.08], size: [0.1, 0.08, 0.03], material: "ear_inner" }));
  part("ear_r_inner", box({ parent: "ear_r", at: [0, -0.02, -0.08], size: [0.1, 0.08, 0.03], material: "ear_inner" }));

  part("whisker_l1", cylinder({ parent: "muzzle", at: [-0.26, 0.04, -0.08], rot: [0, 0, 82], radius: 0.008, length: 0.38, radialSegments: 6, material: "whisker" }));
  part("whisker_l2", cylinder({ parent: "muzzle", at: [-0.26, -0.02, -0.08], rot: [0, 0, 96], radius: 0.008, length: 0.34, radialSegments: 6, material: "whisker" }));
  part("whisker_r1", cylinder({ parent: "muzzle", at: [0.26, 0.04, -0.08], rot: [0, 0, -82], radius: 0.008, length: 0.38, radialSegments: 6, material: "whisker" }));
  part("whisker_r2", cylinder({ parent: "muzzle", at: [0.26, -0.02, -0.08], rot: [0, 0, -96], radius: 0.008, length: 0.34, radialSegments: 6, material: "whisker" }));

  part("leg_fl", capsule({ parent: "body", at: [-0.34, -0.43, -0.5], radius: 0.095, length: 0.56, material: "coat", joint: { pivot: [0, 0.28, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.34, -0.43, -0.5], radius: 0.095, length: 0.56, material: "coat", joint: { pivot: [0, 0.28, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.36, -0.42, 0.5], radius: 0.11, length: 0.58, material: "coat_shadow", joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.36, -0.42, 0.5], radius: 0.11, length: 0.58, material: "coat_shadow", joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] } }));

  part("leg_fl_stripe_1", box({ parent: "leg_fl", at: [0, 0.08, -0.08], rot: [18, 0, 0], size: [0.22, 0.055, 0.08], material: "stripe" }));
  part("leg_fl_stripe_2", box({ parent: "leg_fl", at: [0, -0.1, 0.07], rot: [-18, 0, 0], size: [0.22, 0.055, 0.08], material: "stripe" }));
  part("leg_fr_stripe_1", box({ parent: "leg_fr", at: [0, 0.08, -0.08], rot: [-18, 0, 0], size: [0.22, 0.055, 0.08], material: "stripe" }));
  part("leg_fr_stripe_2", box({ parent: "leg_fr", at: [0, -0.1, 0.07], rot: [18, 0, 0], size: [0.22, 0.055, 0.08], material: "stripe" }));
  part("leg_bl_stripe_1", box({ parent: "leg_bl", at: [0, 0.08, -0.08], rot: [18, 0, 0], size: [0.24, 0.055, 0.08], material: "stripe" }));
  part("leg_bl_stripe_2", box({ parent: "leg_bl", at: [0, -0.1, 0.07], rot: [-18, 0, 0], size: [0.24, 0.055, 0.08], material: "stripe" }));
  part("leg_br_stripe_1", box({ parent: "leg_br", at: [0, 0.08, -0.08], rot: [-18, 0, 0], size: [0.24, 0.055, 0.08], material: "stripe" }));
  part("leg_br_stripe_2", box({ parent: "leg_br", at: [0, -0.1, 0.07], rot: [18, 0, 0], size: [0.24, 0.055, 0.08], material: "stripe" }));

  part("paw_fl", box({ parent: "leg_fl", at: [0, -0.35, -0.07], size: [0.27, 0.095, 0.33], material: "paw" }));
  part("paw_fr", box({ parent: "leg_fr", at: [0, -0.35, -0.07], size: [0.27, 0.095, 0.33], material: "paw" }));
  part("paw_bl", box({ parent: "leg_bl", at: [0, -0.36, 0.02], size: [0.29, 0.1, 0.35], material: "paw" }));
  part("paw_br", box({ parent: "leg_br", at: [0, -0.36, 0.02], size: [0.29, 0.1, 0.35], material: "paw" }));

  part("toe_fl_l", box({ parent: "paw_fl", at: [-0.09, 0.03, -0.13], size: [0.045, 0.025, 0.1], material: "coat_shadow" }));
  part("toe_fl_m", box({ parent: "paw_fl", at: [0, 0.03, -0.14], size: [0.045, 0.025, 0.11], material: "coat_shadow" }));
  part("toe_fl_r", box({ parent: "paw_fl", at: [0.09, 0.03, -0.13], size: [0.045, 0.025, 0.1], material: "coat_shadow" }));
  part("toe_fr_l", box({ parent: "paw_fr", at: [-0.09, 0.03, -0.13], size: [0.045, 0.025, 0.1], material: "coat_shadow" }));
  part("toe_fr_m", box({ parent: "paw_fr", at: [0, 0.03, -0.14], size: [0.045, 0.025, 0.11], material: "coat_shadow" }));
  part("toe_fr_r", box({ parent: "paw_fr", at: [0.09, 0.03, -0.13], size: [0.045, 0.025, 0.1], material: "coat_shadow" }));
  part("toe_bl_l", box({ parent: "paw_bl", at: [-0.1, 0.03, -0.14], size: [0.045, 0.025, 0.1], material: "coat_shadow" }));
  part("toe_bl_m", box({ parent: "paw_bl", at: [0, 0.03, -0.15], size: [0.045, 0.025, 0.11], material: "coat_shadow" }));
  part("toe_bl_r", box({ parent: "paw_bl", at: [0.1, 0.03, -0.14], size: [0.045, 0.025, 0.1], material: "coat_shadow" }));
  part("toe_br_l", box({ parent: "paw_br", at: [-0.1, 0.03, -0.14], size: [0.045, 0.025, 0.1], material: "coat_shadow" }));
  part("toe_br_m", box({ parent: "paw_br", at: [0, 0.03, -0.15], size: [0.045, 0.025, 0.11], material: "coat_shadow" }));
  part("toe_br_r", box({ parent: "paw_br", at: [0.1, 0.03, -0.14], size: [0.045, 0.025, 0.1], material: "coat_shadow" }));

  part("claw_fl_l", box({ parent: "paw_fl", at: [-0.09, -0.01, -0.19], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_fl_m", box({ parent: "paw_fl", at: [0, -0.01, -0.2], size: [0.048, 0.032, 0.1], material: "claw" }));
  part("claw_fl_r", box({ parent: "paw_fl", at: [0.09, -0.01, -0.19], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_fr_l", box({ parent: "paw_fr", at: [-0.09, -0.01, -0.19], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_fr_m", box({ parent: "paw_fr", at: [0, -0.01, -0.2], size: [0.048, 0.032, 0.1], material: "claw" }));
  part("claw_fr_r", box({ parent: "paw_fr", at: [0.09, -0.01, -0.19], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_bl_l", box({ parent: "paw_bl", at: [-0.1, -0.01, -0.2], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_bl_m", box({ parent: "paw_bl", at: [0, -0.01, -0.21], size: [0.048, 0.032, 0.1], material: "claw" }));
  part("claw_bl_r", box({ parent: "paw_bl", at: [0.1, -0.01, -0.2], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_br_l", box({ parent: "paw_br", at: [-0.1, -0.01, -0.2], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_br_m", box({ parent: "paw_br", at: [0, -0.01, -0.21], size: [0.048, 0.032, 0.1], material: "claw" }));
  part("claw_br_r", box({ parent: "paw_br", at: [0.1, -0.01, -0.2], size: [0.048, 0.032, 0.09], material: "claw" }));

  part("tail", capsule({
    parent: "body",
    at: [0, 0.08, 0.9],
    rot: [44, 0, 0],
    radius: 0.065,
    length: 1.02,
    capSegments: 5,
    radialSegments: 14,
    material: "coat",
    joint: { pivot: [0, 0.51, 0], axis: [0, 0, 1] },
  }));
  part("tail_ring_1", capsule({ parent: "tail", at: [0, 0.22, 0.02], radius: 0.068, length: 0.07, radialSegments: 14, material: "stripe" }));
  part("tail_ring_2", capsule({ parent: "tail", at: [0, 0.02, 0.03], radius: 0.069, length: 0.075, radialSegments: 14, material: "stripe" }));
  part("tail_ring_3", capsule({ parent: "tail", at: [0, -0.2, 0.04], radius: 0.071, length: 0.08, radialSegments: 14, material: "stripe" }));
  part("tail_ring_4", capsule({ parent: "tail", at: [0, -0.42, 0.05], radius: 0.073, length: 0.085, radialSegments: 14, material: "stripe" }));
  part("tail_tip", sphere({ parent: "tail", at: [0, -0.55, 0.06], radius: 0.08, material: "stripe" }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.02,
    cycleDistance: 1.02,
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
    bodyBob: 0.011,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.66,
    swingDegrees: 19,
    tail: "tail",
    tailSwingDegrees: 11,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.14 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.75, lag: 0.2 }),
    ],
  });
});
