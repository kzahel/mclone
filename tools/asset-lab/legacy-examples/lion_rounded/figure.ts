import { legacyFigure } from "../../src/dsl";

export default legacyFigure("lion_rounded", ({ mat, asciiTexture, part, box, capsule, sphere, quadrupedWalk, followThrough }) => {
  mat("coat", "#c88d3d");
  mat("coat_light", "#e4bd72");
  mat("coat_shadow", "#9a632e");
  mat("mane", "#5b321d");
  mat("mane_dark", "#2e1a12");
  mat("mane_tip", "#744122");
  mat("ear_inner", "#b76f4b");
  mat("eye", "#d7b44f");
  mat("nose", "#16100c");
  mat("paw", "#6b3d20");
  mat("claw", "#140f0b");

  asciiTexture("face", {
    palette: {
      ".": "#c88d3d",
      "m": "#5b321d",
      "d": "#2e1a12",
      "e": "#d7b44f",
      "c": "#e4bd72",
    },
    pixels: [
      "mmmmmmmm",
      "md....dm",
      "m.e..e.m",
      "m......m",
      "..cccc..",
      ".cccccc.",
      "..cccc..",
      "........",
    ],
  });

  asciiTexture("snout_face", {
    palette: {
      ".": "#e4bd72",
      "s": "#9a632e",
      "n": "#16100c",
      "w": "#f0d99a",
    },
    pixels: [
      "..wwww..",
      ".wwwwww.",
      ".s....s.",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
      "........",
    ],
  });

  part("body", box({ size: [1.02, 0.58, 1.58], material: "coat" }));
  part("back", box({ parent: "body", at: [0, 0.22, 0.08], size: [0.92, 0.18, 1.16], material: "coat_shadow" }));
  part("chest", box({ parent: "body", at: [0, 0.02, -0.64], size: [1.08, 0.58, 0.44], material: "mane_tip" }));
  part("belly", box({ parent: "body", at: [0, -0.31, -0.02], size: [0.72, 0.12, 1.02], material: "coat_light" }));
  part("haunch_l", box({ parent: "body", at: [-0.42, -0.02, 0.48], size: [0.22, 0.42, 0.46], material: "coat_shadow" }));
  part("haunch_r", box({ parent: "body", at: [0.42, -0.02, 0.48], size: [0.22, 0.42, 0.46], material: "coat_shadow" }));
  part("shoulder_l", box({ parent: "body", at: [-0.42, -0.02, -0.5], size: [0.24, 0.48, 0.42], material: "mane" }));
  part("shoulder_r", box({ parent: "body", at: [0.42, -0.02, -0.5], size: [0.24, 0.48, 0.42], material: "mane" }));

  part("neck", box({
    parent: "body",
    at: [0, 0.22, -0.86],
    rot: [-12, 0, 0],
    size: [0.66, 0.5, 0.42],
    material: "mane",
    joint: { pivot: [0, -0.24, 0.16], axis: [1, 0, 0] },
  }));
  part("throat_mane", box({ parent: "neck", at: [0, -0.2, -0.08], size: [0.54, 0.28, 0.28], material: "mane_dark" }));

  part("head", box({
    parent: "neck",
    at: [0, 0.14, -0.38],
    rot: [8, 0, 0],
    size: [0.62, 0.48, 0.5],
    material: "coat",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("mane_back", box({ parent: "head", at: [0, 0.02, 0.16], size: [0.78, 0.62, 0.26], material: "mane" }));
  part("mane_top", box({ parent: "head", at: [0, 0.31, -0.02], size: [0.72, 0.18, 0.44], material: "mane_dark" }));
  part("mane_l", box({ parent: "head", at: [-0.39, 0.02, -0.02], size: [0.18, 0.5, 0.42], material: "mane" }));
  part("mane_r", box({ parent: "head", at: [0.39, 0.02, -0.02], size: [0.18, 0.5, 0.42], material: "mane" }));
  part("mane_beard", box({ parent: "head", at: [0, -0.31, -0.02], size: [0.5, 0.2, 0.38], material: "mane_dark" }));
  part("brow", box({ parent: "head", at: [0, 0.1, -0.24], size: [0.5, 0.1, 0.14], material: "mane_dark" }));
  part("cheek_l", box({ parent: "head", at: [-0.25, -0.08, -0.22], size: [0.16, 0.2, 0.18], material: "coat_light" }));
  part("cheek_r", box({ parent: "head", at: [0.25, -0.08, -0.22], size: [0.16, 0.2, 0.18], material: "coat_light" }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.4],
    size: [0.46, 0.22, 0.3],
    material: "coat_light",
    faces: {
      north: { texture: "snout_face" },
    },
  }));
  part("nose", sphere({ parent: "muzzle", at: [0, 0.02, -0.17], radius: 0.055, material: "nose" }));

  part("ear_l", sphere({ parent: "head", at: [-0.29, 0.25, -0.02], radius: 0.11, material: "mane_dark" }));
  part("ear_r", sphere({ parent: "head", at: [0.29, 0.25, -0.02], radius: 0.11, material: "mane_dark" }));
  part("ear_l_inner", box({ parent: "ear_l", at: [0, -0.02, -0.08], size: [0.1, 0.08, 0.03], material: "ear_inner" }));
  part("ear_r_inner", box({ parent: "ear_r", at: [0, -0.02, -0.08], size: [0.1, 0.08, 0.03], material: "ear_inner" }));

  part("leg_fl", capsule({ parent: "body", at: [-0.34, -0.43, -0.48], radius: 0.1, length: 0.56, material: "coat_shadow", joint: { pivot: [0, 0.28, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.34, -0.43, -0.48], radius: 0.1, length: 0.56, material: "coat_shadow", joint: { pivot: [0, 0.28, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.36, -0.42, 0.48], radius: 0.115, length: 0.58, material: "coat", joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.36, -0.42, 0.48], radius: 0.115, length: 0.58, material: "coat", joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] } }));

  part("paw_fl", box({ parent: "leg_fl", at: [0, -0.35, -0.07], size: [0.26, 0.095, 0.32], material: "paw" }));
  part("paw_fr", box({ parent: "leg_fr", at: [0, -0.35, -0.07], size: [0.26, 0.095, 0.32], material: "paw" }));
  part("paw_bl", box({ parent: "leg_bl", at: [0, -0.36, 0.02], size: [0.28, 0.1, 0.34], material: "paw" }));
  part("paw_br", box({ parent: "leg_br", at: [0, -0.36, 0.02], size: [0.28, 0.1, 0.34], material: "paw" }));

  part("claw_fl_l", box({ parent: "paw_fl", at: [-0.08, -0.01, -0.18], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_fl_m", box({ parent: "paw_fl", at: [0, -0.01, -0.19], size: [0.048, 0.032, 0.1], material: "claw" }));
  part("claw_fl_r", box({ parent: "paw_fl", at: [0.08, -0.01, -0.18], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_fr_l", box({ parent: "paw_fr", at: [-0.08, -0.01, -0.18], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_fr_m", box({ parent: "paw_fr", at: [0, -0.01, -0.19], size: [0.048, 0.032, 0.1], material: "claw" }));
  part("claw_fr_r", box({ parent: "paw_fr", at: [0.08, -0.01, -0.18], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_bl_l", box({ parent: "paw_bl", at: [-0.09, -0.01, -0.19], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_bl_m", box({ parent: "paw_bl", at: [0, -0.01, -0.2], size: [0.048, 0.032, 0.1], material: "claw" }));
  part("claw_bl_r", box({ parent: "paw_bl", at: [0.09, -0.01, -0.19], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_br_l", box({ parent: "paw_br", at: [-0.09, -0.01, -0.19], size: [0.048, 0.032, 0.09], material: "claw" }));
  part("claw_br_m", box({ parent: "paw_br", at: [0, -0.01, -0.2], size: [0.048, 0.032, 0.1], material: "claw" }));
  part("claw_br_r", box({ parent: "paw_br", at: [0.09, -0.01, -0.19], size: [0.048, 0.032, 0.09], material: "claw" }));

  part("tail", capsule({
    parent: "body",
    at: [0, 0.08, 0.86],
    rot: [46, 0, 0],
    radius: 0.045,
    length: 0.82,
    material: "coat",
    joint: { pivot: [0, 0.41, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", capsule({ parent: "tail", at: [0, -0.5, 0.06], rot: [-10, 0, 0], radius: 0.115, length: 0.28, material: "mane_dark" }));
  part("tail_tuft_tip", sphere({ parent: "tail_tuft", at: [0, -0.15, 0.02], radius: 0.09, material: "mane" }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.05,
    cycleDistance: 0.96,
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
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 9,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.14 }),
      followThrough("mane_beard", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.18 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.2 }),
    ],
  });
});
