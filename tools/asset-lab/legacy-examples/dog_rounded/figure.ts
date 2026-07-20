import { legacyFigure } from "../../src/dsl";

export default legacyFigure("dog_rounded", ({ mat, asciiTexture, part, box, capsule, sphere, quadrupedWalk }) => {
  mat("fur", "#8b5f3e");
  mat("fur_dark", "#5b3928");
  mat("fur_light", "#c39161");
  mat("nose", "#16110f");
  mat("collar", "#2f69b1");
  mat("paw", "#3a2720");

  asciiTexture("face", {
    palette: {
      ".": "#8b5f3e",
      "e": "#120d0b",
      "m": "#c39161",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "..mmmm..",
      "..mmmm..",
      "........",
      "........",
    ],
  });

  asciiTexture("snout", {
    palette: {
      ".": "#c39161",
      "n": "#120d0b",
    },
    pixels: [
      "........",
      "........",
      "...nn...",
      "...nn...",
      "........",
      "........",
      "........",
      "........",
    ],
  });

  part("body", box({ size: [1.16, 0.58, 0.9], material: "fur" }));
  part("chest", box({ parent: "body", at: [0, -0.02, -0.42], size: [1.04, 0.52, 0.18], material: "fur_light" }));
  part("neck", box({ parent: "body", at: [0, 0.12, -0.58], size: [0.48, 0.32, 0.22], material: "fur" }));
  part("collar", box({ parent: "neck", at: [0, -0.04, -0.03], size: [0.52, 0.08, 0.26], material: "collar" }));

  part("head", box({
    parent: "neck",
    at: [0, 0.16, -0.32],
    size: [0.52, 0.44, 0.46],
    material: "fur",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.08, -0.32],
    size: [0.34, 0.18, 0.22],
    material: "fur_light",
    faces: {
      north: { texture: "snout" },
    },
  }));
  part("nose", sphere({ parent: "snout", at: [0, 0.02, -0.14], radius: 0.055, material: "nose" }));
  part("ear_l", box({ parent: "head", at: [-0.25, 0.18, -0.02], rot: [0, 0, -18], size: [0.14, 0.34, 0.1], material: "fur_dark" }));
  part("ear_r", box({ parent: "head", at: [0.25, 0.18, -0.02], rot: [0, 0, 18], size: [0.14, 0.34, 0.1], material: "fur_dark" }));

  part("leg_fl", capsule({ parent: "body", at: [-0.36, -0.48, -0.3], radius: 0.07, length: 0.44, material: "fur", joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.36, -0.48, -0.3], radius: 0.07, length: 0.44, material: "fur", joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.36, -0.48, 0.28], radius: 0.075, length: 0.45, material: "fur_dark", joint: { pivot: [0, 0.225, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.36, -0.48, 0.28], radius: 0.075, length: 0.45, material: "fur_dark", joint: { pivot: [0, 0.225, 0], axis: [1, 0, 0] } }));

  part("paw_fl", box({ parent: "leg_fl", at: [0, -0.27, -0.04], size: [0.16, 0.08, 0.2], material: "paw" }));
  part("paw_fr", box({ parent: "leg_fr", at: [0, -0.27, -0.04], size: [0.16, 0.08, 0.2], material: "paw" }));
  part("paw_bl", box({ parent: "leg_bl", at: [0, -0.28, 0.02], size: [0.16, 0.08, 0.2], material: "paw" }));
  part("paw_br", box({ parent: "leg_br", at: [0, -0.28, 0.02], size: [0.16, 0.08, 0.2], material: "paw" }));

  part("tail", capsule({
    parent: "body",
    at: [0, 0.2, 0.58],
    rot: [55, 0, 0],
    radius: 0.055,
    length: 0.48,
    material: "fur_dark",
    joint: { pivot: [0, -0.24, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 0.9,
    cycleDistance: 0.88,
    gait: "trot",
    loop: true,
    samples: 11,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 3,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.56,
    swingDegrees: 22,
    tail: "tail",
    tailSwingDegrees: 16,
  });
});
