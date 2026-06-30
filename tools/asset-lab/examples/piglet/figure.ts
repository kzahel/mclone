import { figure } from "../../src/dsl";

export default figure("piglet", ({ mat, asciiTexture, part, box, capsule, sphere, quadrupedWalk, followThrough }) => {
  mat("skin", "#d88a92");
  mat("skin_dark", "#bd6f7b");
  mat("hoof", "#4a3033");

  asciiTexture("head_face", {
    palette: {
      ".": "#d88a92",
      "e": "#261718",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  });

  asciiTexture("snout_face", {
    palette: {
      ".": "#bd6f7b",
      "n": "#261718",
    },
    pixels: [
      "........",
      "........",
      "..n..n..",
      "..n..n..",
      "........",
      "........",
      "........",
      "........",
    ],
  });

  part("body", box({ size: [1.18, 0.68, 0.78], material: "skin" }));
  part("head", box({
    parent: "body",
    at: [0, 0.17, -0.68],
    size: [0.7, 0.58, 0.58],
    material: "skin",
    faces: {
      north: { texture: "head_face" },
    },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.06, -0.34],
    size: [0.36, 0.2, 0.16],
    material: "skin_dark",
    faces: {
      north: { texture: "snout_face" },
    },
  }));

  part("ear_l", box({ parent: "head", at: [-0.29, 0.32, -0.08], rot: [0, 0, -10], size: [0.16, 0.22, 0.08], material: "skin" }));
  part("ear_r", box({ parent: "head", at: [0.29, 0.32, -0.08], rot: [0, 0, 10], size: [0.16, 0.22, 0.08], material: "skin" }));

  part("leg_fl", capsule({ parent: "body", at: [-0.42, -0.53, -0.26], radius: 0.09, length: 0.42, material: "skin", joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.42, -0.53, -0.26], radius: 0.09, length: 0.42, material: "skin", joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.42, -0.53, 0.28], radius: 0.09, length: 0.42, material: "skin", joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.42, -0.53, 0.28], radius: 0.09, length: 0.42, material: "skin", joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] } }));

  part("hoof_fl", box({ parent: "leg_fl", at: [0, -0.28, 0], size: [0.16, 0.08, 0.16], material: "hoof" }));
  part("hoof_fr", box({ parent: "leg_fr", at: [0, -0.28, 0], size: [0.16, 0.08, 0.16], material: "hoof" }));
  part("hoof_bl", box({ parent: "leg_bl", at: [0, -0.28, 0], size: [0.16, 0.08, 0.16], material: "hoof" }));
  part("hoof_br", box({ parent: "leg_br", at: [0, -0.28, 0], size: [0.16, 0.08, 0.16], material: "hoof" }));

  part("tail", sphere({ parent: "body", at: [0, 0.14, 0.47], radius: 0.09, material: "skin_dark" }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1,
    cycleDistance: 0.72,
    gait: "trot",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.0125,
    bodyBobCenter: 0.0125,
    head: "head",
    headSwingDegrees: 4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.56,
    swingDegrees: 20,
    tail: "tail",
    tailSwingDegrees: 15,
    tracks: [
      // Ears flop a beat behind the body bob, with a little overshoot.
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 17, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 17, overshoot: 0.6, lag: 0.12 }),
    ],
  });
});
