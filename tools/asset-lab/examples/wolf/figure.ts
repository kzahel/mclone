import { figure } from "../../src/dsl";

export default figure("wolf", ({ mat, asciiTexture, part, box, capsule, sphere, quadrupedWalk, followThrough }) => {
  mat("coat", "#6d7479");
  mat("coat_dark", "#2f3438");
  mat("coat_mid", "#535b60");
  mat("coat_light", "#c7c2b6");
  mat("ear_inner", "#8f8378");
  mat("eye", "#d7b15a");
  mat("nose", "#111315");
  mat("paw", "#242729");

  asciiTexture("face", {
    palette: {
      ".": "#6d7479",
      "d": "#2f3438",
      "e": "#d7b15a",
      "p": "#c7c2b6",
    },
    pixels: [
      "dd....dd",
      "de....ed",
      ".e....e.",
      "..pppp..",
      "..pppp..",
      "...pp...",
      "........",
      "........",
    ],
  });

  asciiTexture("snout_face", {
    palette: {
      ".": "#c7c2b6",
      "d": "#535b60",
      "n": "#111315",
    },
    pixels: [
      "..dddd..",
      ".dddddd.",
      "...nn...",
      "...nn...",
      "........",
      "........",
      "........",
      "........",
    ],
  });

  part("body", box({ size: [0.74, 0.48, 1.42], material: "coat_mid" }));
  part("saddle", box({ parent: "body", at: [0, 0.18, 0.08], size: [0.72, 0.16, 1.02], material: "coat_dark" }));
  part("chest", box({ parent: "body", at: [0, -0.02, -0.58], size: [0.6, 0.42, 0.24], material: "coat_light" }));
  part("belly", box({ parent: "body", at: [0, -0.22, -0.02], size: [0.54, 0.12, 0.86], material: "coat_light" }));
  part("haunch_l", box({ parent: "body", at: [-0.32, 0.02, 0.48], size: [0.18, 0.42, 0.38], material: "coat" }));
  part("haunch_r", box({ parent: "body", at: [0.32, 0.02, 0.48], size: [0.18, 0.42, 0.38], material: "coat" }));

  part("neck", box({
    parent: "body",
    at: [0, 0.18, -0.78],
    rot: [-18, 0, 0],
    size: [0.42, 0.42, 0.36],
    material: "coat",
    joint: { pivot: [0, -0.2, 0.15], axis: [1, 0, 0] },
  }));
  part("throat", box({ parent: "neck", at: [0, -0.12, -0.1], size: [0.3, 0.18, 0.24], material: "coat_light" }));

  part("head", box({
    parent: "neck",
    at: [0, 0.16, -0.34],
    rot: [10, 0, 0],
    size: [0.46, 0.38, 0.44],
    material: "coat",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("brow", box({ parent: "head", at: [0, 0.1, -0.2], size: [0.5, 0.12, 0.16], material: "coat_dark" }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.08, -0.36],
    size: [0.3, 0.2, 0.34],
    material: "coat_light",
    faces: {
      north: { texture: "snout_face" },
    },
  }));
  part("nose", sphere({ parent: "muzzle", at: [0, 0.02, -0.19], radius: 0.045, material: "nose" }));

  part("ear_l", box({ parent: "head", at: [-0.18, 0.28, -0.02], rot: [-8, 0, -14], size: [0.14, 0.34, 0.1], material: "coat_dark" }));
  part("ear_r", box({ parent: "head", at: [0.18, 0.28, -0.02], rot: [-8, 0, 14], size: [0.14, 0.34, 0.1], material: "coat_dark" }));
  part("ear_l_inner", box({ parent: "ear_l", at: [0, -0.02, -0.055], size: [0.07, 0.22, 0.025], material: "ear_inner" }));
  part("ear_r_inner", box({ parent: "ear_r", at: [0, -0.02, -0.055], size: [0.07, 0.22, 0.025], material: "ear_inner" }));

  part("leg_fl", capsule({ parent: "body", at: [-0.24, -0.42, -0.42], radius: 0.06, length: 0.5, material: "coat", joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.24, -0.42, -0.42], radius: 0.06, length: 0.5, material: "coat", joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.26, -0.42, 0.46], radius: 0.065, length: 0.52, material: "coat_dark", joint: { pivot: [0, 0.26, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.26, -0.42, 0.46], radius: 0.065, length: 0.52, material: "coat_dark", joint: { pivot: [0, 0.26, 0], axis: [1, 0, 0] } }));

  part("sock_fl", box({ parent: "leg_fl", at: [0, -0.24, 0], size: [0.12, 0.16, 0.12], material: "coat_light" }));
  part("sock_fr", box({ parent: "leg_fr", at: [0, -0.24, 0], size: [0.12, 0.16, 0.12], material: "coat_light" }));
  part("sock_bl", box({ parent: "leg_bl", at: [0, -0.25, 0], size: [0.13, 0.16, 0.13], material: "coat_mid" }));
  part("sock_br", box({ parent: "leg_br", at: [0, -0.25, 0], size: [0.13, 0.16, 0.13], material: "coat_mid" }));

  part("paw_fl", box({ parent: "leg_fl", at: [0, -0.31, -0.05], size: [0.16, 0.075, 0.22], material: "paw" }));
  part("paw_fr", box({ parent: "leg_fr", at: [0, -0.31, -0.05], size: [0.16, 0.075, 0.22], material: "paw" }));
  part("paw_bl", box({ parent: "leg_bl", at: [0, -0.32, 0.03], size: [0.17, 0.075, 0.23], material: "paw" }));
  part("paw_br", box({ parent: "leg_br", at: [0, -0.32, 0.03], size: [0.17, 0.075, 0.23], material: "paw" }));

  part("tail", capsule({
    parent: "body",
    at: [0, 0.08, 0.82],
    rot: [38, 0, 0],
    radius: 0.085,
    length: 0.74,
    material: "coat_dark",
    joint: { pivot: [0, 0.37, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", capsule({ parent: "tail", at: [0, -0.42, 0.06], rot: [-10, 0, 0], radius: 0.09, length: 0.3, material: "coat_mid" }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 0.86,
    cycleDistance: 0.94,
    gait: "trot",
    loop: true,
    samples: 13,
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
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.58,
    swingDegrees: 23,
    tail: "tail",
    tailSwingDegrees: 13,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.1 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.7, lag: 0.18 }),
    ],
  });
});
