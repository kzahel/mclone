import { figure } from "../../src/dsl";

export default figure("player", ({ mat, asciiTexture, part, box, bipedWalk }) => {
  mat("skin", "#c58b64");
  mat("hair", "#3b2418");
  mat("shirt", "#2878b8");
  mat("shirt_dark", "#1d5b8c");
  mat("pants", "#34495e");
  mat("boot", "#2a1e1a");

  asciiTexture("face", {
    palette: {
      ".": "#c58b64",
      "e": "#19120e",
      "h": "#3b2418",
      "m": "#7f4f3b",
    },
    pixels: [
      "hhhhhhhh",
      "h......h",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "...mm...",
      "........",
      "........",
    ],
  });

  part("torso", box({ at: [0, 0.78, 0], size: [0.54, 0.72, 0.28], material: "shirt" }));
  part("shirt_band", box({ parent: "torso", at: [0, -0.29, -0.002], size: [0.56, 0.08, 0.3], material: "shirt_dark" }));
  part("head", box({
    parent: "torso",
    at: [0, 0.62, 0],
    size: [0.46, 0.46, 0.46],
    material: "skin",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("hair_cap", box({ parent: "head", at: [0, 0.18, 0], size: [0.48, 0.12, 0.48], material: "hair" }));

  part("arm_l", box({ parent: "torso", at: [-0.42, 0.1, 0], size: [0.18, 0.62, 0.2], material: "skin", joint: { pivot: [0, 0.31, 0], axis: [1, 0, 0] } }));
  part("arm_r", box({ parent: "torso", at: [0.42, 0.1, 0], size: [0.18, 0.62, 0.2], material: "skin", joint: { pivot: [0, 0.31, 0], axis: [1, 0, 0] } }));
  part("sleeve_l", box({ parent: "arm_l", at: [0, 0.21, 0], size: [0.19, 0.18, 0.21], material: "shirt" }));
  part("sleeve_r", box({ parent: "arm_r", at: [0, 0.21, 0], size: [0.19, 0.18, 0.21], material: "shirt" }));

  part("leg_l", box({ parent: "torso", at: [-0.15, -0.7, 0], size: [0.2, 0.68, 0.22], material: "pants", joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] } }));
  part("leg_r", box({ parent: "torso", at: [0.15, -0.7, 0], size: [0.2, 0.68, 0.22], material: "pants", joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] } }));
  part("foot_l", box({ parent: "leg_l", at: [0, -0.38, -0.04], size: [0.22, 0.1, 0.3], material: "boot" }));
  part("foot_r", box({ parent: "leg_r", at: [0, -0.38, -0.04], size: [0.22, 0.1, 0.3], material: "boot" }));

  bipedWalk("walk", {
    fps: 12,
    duration: 0.9,
    cycleDistance: 0.86,
    loop: true,
    samples: 11,
    body: "torso",
    bodyBob: 0.018,
    bodyBobCenter: 0.018,
    head: "head",
    headSwingDegrees: 2,
    leftArm: "arm_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "arm_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.62,
    swingDegrees: 24,
  });
});
