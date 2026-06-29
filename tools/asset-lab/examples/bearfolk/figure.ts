import { figure } from "../../src/dsl";

export default figure("bearfolk", ({ mat, asciiTexture, part, box, capsule, sphere, bipedWalk }) => {
  mat("fur", "#6d4a32");
  mat("fur_dark", "#3c281d");
  mat("fur_light", "#b28663");
  mat("cloth", "#436b57");
  mat("claw", "#1b1512");

  asciiTexture("face", {
    palette: {
      ".": "#6d4a32",
      "e": "#11100e",
      "m": "#b28663",
      "n": "#1b1512",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "..mmmm..",
      "..mnmm..",
      "..mmmm..",
      "........",
      "........",
    ],
  });

  part("torso", capsule({ at: [0, 0.9, 0], radius: 0.32, length: 0.58, material: "fur" }));
  part("vest", box({ parent: "torso", at: [0, 0.02, -0.08], size: [0.6, 0.58, 0.16], material: "cloth" }));
  part("belly", sphere({ parent: "torso", at: [0, -0.08, -0.13], radius: 0.24, material: "fur_light" }));

  part("head", sphere({ parent: "torso", at: [0, 0.57, -0.02], radius: 0.28, material: "fur" }));
  part("face_patch", box({
    parent: "head",
    at: [0, -0.02, -0.23],
    size: [0.32, 0.24, 0.08],
    material: "fur_light",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("ear_l", sphere({ parent: "head", at: [-0.22, 0.18, -0.02], radius: 0.11, material: "fur_dark" }));
  part("ear_r", sphere({ parent: "head", at: [0.22, 0.18, -0.02], radius: 0.11, material: "fur_dark" }));

  part("arm_l", capsule({ parent: "torso", at: [-0.42, 0.08, 0], radius: 0.095, length: 0.72, material: "fur", joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] } }));
  part("arm_r", capsule({ parent: "torso", at: [0.42, 0.08, 0], radius: 0.095, length: 0.72, material: "fur", joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] } }));
  part("paw_l", sphere({ parent: "arm_l", at: [0, -0.42, -0.02], radius: 0.12, material: "fur_dark" }));
  part("paw_r", sphere({ parent: "arm_r", at: [0, -0.42, -0.02], radius: 0.12, material: "fur_dark" }));
  part("claw_l", box({ parent: "paw_l", at: [0, -0.05, -0.11], size: [0.12, 0.04, 0.06], material: "claw" }));
  part("claw_r", box({ parent: "paw_r", at: [0, -0.05, -0.11], size: [0.12, 0.04, 0.06], material: "claw" }));

  part("leg_l", capsule({ parent: "torso", at: [-0.18, -0.57, 0], radius: 0.11, length: 0.62, material: "fur_dark", joint: { pivot: [0, 0.31, 0], axis: [1, 0, 0] } }));
  part("leg_r", capsule({ parent: "torso", at: [0.18, -0.57, 0], radius: 0.11, length: 0.62, material: "fur_dark", joint: { pivot: [0, 0.31, 0], axis: [1, 0, 0] } }));
  part("foot_l", box({ parent: "leg_l", at: [0, -0.37, -0.07], size: [0.26, 0.1, 0.34], material: "fur_dark" }));
  part("foot_r", box({ parent: "leg_r", at: [0, -0.37, -0.07], size: [0.26, 0.1, 0.34], material: "fur_dark" }));

  bipedWalk("walk", {
    fps: 12,
    duration: 1.1,
    cycleDistance: 0.74,
    loop: true,
    samples: 11,
    armSwingDegrees: 12,
    body: "torso",
    bodyBob: 0.013,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 1.5,
    leftArm: "arm_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "arm_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.66,
    swingDegrees: 18,
  });
});
