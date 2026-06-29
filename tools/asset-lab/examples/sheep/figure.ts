import { figure } from "../../src/dsl";

export default figure("sheep", ({ mat, asciiTexture, part, box, capsule, sphere, quadrupedWalk }) => {
  mat("wool", "#e8ece8");
  mat("wool_shadow", "#cfd7d0");
  mat("skin", "#6f5b4f");
  mat("hoof", "#302826");

  asciiTexture("face", {
    palette: {
      ".": "#6f5b4f",
      "e": "#16110f",
      "m": "#3f312b",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "...mm...",
      "...mm...",
      "........",
      "........",
    ],
  });

  part("body", box({ size: [1.22, 0.76, 0.9], material: "wool" }));
  part("wool_top", box({ parent: "body", at: [0, 0.43, 0], size: [1.12, 0.18, 0.82], material: "wool_shadow" }));
  part("wool_left", sphere({ parent: "body", at: [-0.62, 0.05, -0.2], radius: 0.18, material: "wool" }));
  part("wool_right", sphere({ parent: "body", at: [0.62, 0.05, 0.18], radius: 0.18, material: "wool" }));

  part("head", box({
    parent: "body",
    at: [0, 0.1, -0.72],
    size: [0.54, 0.48, 0.48],
    material: "skin",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("forelock", box({ parent: "head", at: [0, 0.29, -0.04], size: [0.48, 0.16, 0.34], material: "wool" }));
  part("ear_l", box({ parent: "head", at: [-0.33, 0.06, -0.02], rot: [0, 0, -8], size: [0.18, 0.12, 0.08], material: "skin" }));
  part("ear_r", box({ parent: "head", at: [0.33, 0.06, -0.02], rot: [0, 0, 8], size: [0.18, 0.12, 0.08], material: "skin" }));

  part("leg_fl", capsule({ parent: "body", at: [-0.38, -0.56, -0.28], radius: 0.08, length: 0.46, material: "skin", joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.38, -0.56, -0.28], radius: 0.08, length: 0.46, material: "skin", joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.38, -0.56, 0.3], radius: 0.08, length: 0.46, material: "skin", joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.38, -0.56, 0.3], radius: 0.08, length: 0.46, material: "skin", joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] } }));

  part("hoof_fl", box({ parent: "leg_fl", at: [0, -0.3, 0], size: [0.15, 0.08, 0.15], material: "hoof" }));
  part("hoof_fr", box({ parent: "leg_fr", at: [0, -0.3, 0], size: [0.15, 0.08, 0.15], material: "hoof" }));
  part("hoof_bl", box({ parent: "leg_bl", at: [0, -0.3, 0], size: [0.15, 0.08, 0.15], material: "hoof" }));
  part("hoof_br", box({ parent: "leg_br", at: [0, -0.3, 0], size: [0.15, 0.08, 0.15], material: "hoof" }));

  part("tail", sphere({ parent: "body", at: [0, 0.13, 0.52], radius: 0.11, material: "wool_shadow" }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.05,
    cycleDistance: 0.68,
    gait: "walk",
    loop: true,
    samples: 11,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
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
    stanceRatio: 0.64,
    swingDegrees: 16,
    tail: "tail",
    tailSwingDegrees: 5,
  });
});
