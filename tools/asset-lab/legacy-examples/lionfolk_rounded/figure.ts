import { legacyFigure } from "../../src/dsl";

export default legacyFigure("lionfolk_rounded", ({ mat, asciiTexture, part, box, capsule, sphere, bipedWalk, swing }) => {
  mat("fur", "#c88a3d");
  mat("fur_light", "#e0b26c");
  mat("mane", "#6d3d1f");
  mat("mane_dark", "#3d2318");
  mat("cloth", "#7c3f45");
  mat("paw", "#4a2c1d");

  asciiTexture("face", {
    palette: {
      ".": "#c88a3d",
      "e": "#14100d",
      "m": "#e0b26c",
      "n": "#4a2c1d",
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

  part("torso", box({ at: [0, 0.86, 0], size: [0.56, 0.76, 0.3], material: "fur" }));
  part("tunic", box({ parent: "torso", at: [0, -0.04, -0.02], size: [0.6, 0.5, 0.32], material: "cloth" }));
  part("head", sphere({ parent: "torso", at: [0, 0.64, -0.02], radius: 0.25, material: "fur" }));
  part("mane_back", sphere({ parent: "head", at: [0, 0.01, 0.07], radius: 0.32, material: "mane" }));
  part("mane_top", box({ parent: "head", at: [0, 0.23, -0.02], size: [0.42, 0.14, 0.34], material: "mane_dark" }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.04, -0.23],
    size: [0.32, 0.18, 0.1],
    material: "fur_light",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("ear_l", sphere({ parent: "head", at: [-0.2, 0.17, -0.02], radius: 0.08, material: "mane" }));
  part("ear_r", sphere({ parent: "head", at: [0.2, 0.17, -0.02], radius: 0.08, material: "mane" }));

  part("arm_l", capsule({ parent: "torso", at: [-0.39, 0.1, 0], radius: 0.075, length: 0.66, material: "fur", joint: { pivot: [0, 0.33, 0], axis: [1, 0, 0] } }));
  part("arm_r", capsule({ parent: "torso", at: [0.39, 0.1, 0], radius: 0.075, length: 0.66, material: "fur", joint: { pivot: [0, 0.33, 0], axis: [1, 0, 0] } }));
  part("hand_l", sphere({ parent: "arm_l", at: [0, -0.39, -0.02], radius: 0.09, material: "paw" }));
  part("hand_r", sphere({ parent: "arm_r", at: [0, -0.39, -0.02], radius: 0.09, material: "paw" }));

  part("leg_l", capsule({ parent: "torso", at: [-0.17, -0.66, 0], radius: 0.085, length: 0.68, material: "fur", joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] } }));
  part("leg_r", capsule({ parent: "torso", at: [0.17, -0.66, 0], radius: 0.085, length: 0.68, material: "fur", joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] } }));
  part("foot_l", box({ parent: "leg_l", at: [0, -0.4, -0.06], size: [0.22, 0.08, 0.32], material: "paw" }));
  part("foot_r", box({ parent: "leg_r", at: [0, -0.4, -0.06], size: [0.22, 0.08, 0.32], material: "paw" }));

  part("tail", capsule({
    parent: "torso",
    at: [0, -0.18, 0.24],
    rot: [-35, 0, 0],
    radius: 0.035,
    length: 0.62,
    material: "fur",
    joint: { pivot: [0, 0.31, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", sphere({ parent: "tail", at: [0, -0.36, 0], radius: 0.08, material: "mane" }));

  bipedWalk("walk", {
    fps: 12,
    duration: 0.88,
    cycleDistance: 0.84,
    loop: true,
    samples: 11,
    armSwingDegrees: 16,
    body: "torso",
    bodyBob: 0.016,
    bodyBobCenter: 0.016,
    head: "head",
    headSwingDegrees: 2,
    leftArm: "arm_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "arm_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.62,
    swingDegrees: 22,
    tracks: [
      swing("tail", { axis: "z", degrees: 9, phase: 0.5 }),
    ],
  });
});
