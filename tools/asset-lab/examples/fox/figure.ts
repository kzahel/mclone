import { figure } from "../../src/dsl";

export default figure("fox", ({ mat, asciiTexture, part, box, capsule, sphere, cylinder, quadrupedWalk, followThrough }) => {
  mat("coat", "#d96a22");
  mat("coat_shadow", "#a94316");
  mat("coat_dark", "#5a2414");
  mat("cream", "#f3ead8");
  mat("white", "#fff8e8");
  mat("ear_inner", "#cf7c63");
  mat("eye", "#f0c34e");
  mat("nose", "#12100e");
  mat("black", "#171310");

  asciiTexture("face", {
    palette: {
      ".": "#d96a22",
      "d": "#5a2414",
      "e": "#f0c34e",
      "w": "#fff8e8",
    },
    pixels: [
      "dd....dd",
      ".e....e.",
      ".e....e.",
      "..wwww..",
      ".ww..ww.",
      ".wwwwww.",
      "..wwww..",
      "........",
    ],
  });

  asciiTexture("snout_face", {
    palette: {
      ".": "#fff8e8",
      "n": "#12100e",
      "s": "#d96a22",
    },
    pixels: [
      "........",
      "..ssss..",
      "..ssss..",
      "...nn...",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
    ],
  });

  part("body", box({ size: [0.58, 0.38, 1.16], material: "coat" }));
  part("saddle", box({ parent: "body", at: [0, 0.15, 0.05], size: [0.54, 0.1, 0.76], material: "coat_shadow" }));
  part("chest", box({ parent: "body", at: [0, -0.02, -0.52], size: [0.42, 0.34, 0.24], material: "white" }));
  part("belly", box({ parent: "body", at: [0, -0.19, -0.06], size: [0.38, 0.08, 0.62], material: "cream" }));
  part("haunch_l", box({ parent: "body", at: [-0.24, 0.0, 0.38], size: [0.14, 0.32, 0.32], material: "coat_shadow" }));
  part("haunch_r", box({ parent: "body", at: [0.24, 0.0, 0.38], size: [0.14, 0.32, 0.32], material: "coat_shadow" }));

  part("neck", box({
    parent: "body",
    at: [0, 0.13, -0.68],
    rot: [-17, 0, 0],
    size: [0.3, 0.3, 0.28],
    material: "coat",
    joint: { pivot: [0, -0.15, 0.12], axis: [1, 0, 0] },
  }));
  part("throat", box({ parent: "neck", at: [0, -0.1, -0.09], size: [0.22, 0.2, 0.2], material: "white" }));

  part("head", box({
    parent: "neck",
    at: [0, 0.12, -0.29],
    rot: [9, 0, 0],
    size: [0.38, 0.32, 0.36],
    material: "coat",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("cheek_l", box({ parent: "head", at: [-0.16, -0.06, -0.18], size: [0.1, 0.16, 0.12], material: "white" }));
  part("cheek_r", box({ parent: "head", at: [0.16, -0.06, -0.18], size: [0.1, 0.16, 0.12], material: "white" }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.07, -0.31],
    size: [0.26, 0.16, 0.26],
    material: "white",
    faces: {
      north: { texture: "snout_face" },
    },
  }));
  part("nose", sphere({ parent: "muzzle", at: [0, 0.02, -0.15], radius: 0.035, material: "nose" }));

  part("ear_l", cylinder({
    parent: "head",
    at: [-0.15, 0.27, -0.01],
    rot: [-6, 0, -16],
    radiusTop: 0.025,
    radiusBottom: 0.09,
    length: 0.34,
    radialSegments: 3,
    material: "coat_dark",
  }));
  part("ear_r", cylinder({
    parent: "head",
    at: [0.15, 0.27, -0.01],
    rot: [-6, 0, 16],
    radiusTop: 0.025,
    radiusBottom: 0.09,
    length: 0.34,
    radialSegments: 3,
    material: "coat_dark",
  }));
  part("ear_l_inner", box({ parent: "ear_l", at: [0, -0.02, -0.075], size: [0.065, 0.2, 0.025], material: "ear_inner" }));
  part("ear_r_inner", box({ parent: "ear_r", at: [0, -0.02, -0.075], size: [0.065, 0.2, 0.025], material: "ear_inner" }));

  part("leg_fl", capsule({ parent: "body", at: [-0.19, -0.34, -0.34], radius: 0.045, length: 0.42, material: "coat", joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.19, -0.34, -0.34], radius: 0.045, length: 0.42, material: "coat", joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.21, -0.35, 0.4], radius: 0.05, length: 0.44, material: "coat_shadow", joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.21, -0.35, 0.4], radius: 0.05, length: 0.44, material: "coat_shadow", joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] } }));

  part("stocking_fl", box({ parent: "leg_fl", at: [0, -0.18, 0], size: [0.09, 0.22, 0.09], material: "black" }));
  part("stocking_fr", box({ parent: "leg_fr", at: [0, -0.18, 0], size: [0.09, 0.22, 0.09], material: "black" }));
  part("stocking_bl", box({ parent: "leg_bl", at: [0, -0.19, 0], size: [0.1, 0.23, 0.1], material: "black" }));
  part("stocking_br", box({ parent: "leg_br", at: [0, -0.19, 0], size: [0.1, 0.23, 0.1], material: "black" }));

  part("paw_fl", box({ parent: "leg_fl", at: [0, -0.27, -0.04], size: [0.13, 0.06, 0.17], material: "black" }));
  part("paw_fr", box({ parent: "leg_fr", at: [0, -0.27, -0.04], size: [0.13, 0.06, 0.17], material: "black" }));
  part("paw_bl", box({ parent: "leg_bl", at: [0, -0.28, 0.02], size: [0.14, 0.06, 0.18], material: "black" }));
  part("paw_br", box({ parent: "leg_br", at: [0, -0.28, 0.02], size: [0.14, 0.06, 0.18], material: "black" }));

  part("tail", capsule({
    parent: "body",
    at: [0, 0.09, 0.68],
    rot: [48, 0, 0],
    radius: 0.12,
    length: 0.78,
    capSegments: 5,
    radialSegments: 14,
    material: "coat",
    joint: { pivot: [0, 0.39, 0], axis: [0, 0, 1] },
  }));
  part("tail_shadow", capsule({ parent: "tail", at: [0, -0.2, 0.08], rot: [-8, 0, 0], radius: 0.08, length: 0.5, material: "coat_shadow" }));
  part("tail_tip", capsule({ parent: "tail", at: [0, -0.54, 0.06], rot: [-8, 0, 0], radius: 0.12, length: 0.28, material: "white" }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 0.82,
    cycleDistance: 0.86,
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
    bodyBob: 0.01,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.56,
    swingDegrees: 24,
    tail: "tail",
    tailSwingDegrees: 16,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.1 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.8, lag: 0.18 }),
    ],
  });
});
