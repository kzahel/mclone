import { figure } from "../../src/dsl";

export default figure("piglet", ({ mat, asciiTexture, part, box, capsule, sphere, clip }) => {
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

  clip("walk", {
    fps: 12,
    loop: true,
    keys: [
      ["body", 0.0, { at: [0, 0.0, 0] }],
      ["body", 0.5, { at: [0, 0.025, 0] }],
      ["body", 1.0, { at: [0, 0.0, 0] }],

      ["head", 0.0, { rot: [0, -4, 0] }],
      ["head", 0.5, { rot: [0, 4, 0] }],
      ["head", 1.0, { rot: [0, -4, 0] }],

      ["leg_fl", 0.0, { rot: [22, 0, 0] }],
      ["leg_fl", 0.5, { rot: [-22, 0, 0] }],
      ["leg_fl", 1.0, { rot: [22, 0, 0] }],
      ["leg_br", 0.0, { rot: [22, 0, 0] }],
      ["leg_br", 0.5, { rot: [-22, 0, 0] }],
      ["leg_br", 1.0, { rot: [22, 0, 0] }],

      ["leg_fr", 0.0, { rot: [-22, 0, 0] }],
      ["leg_fr", 0.5, { rot: [22, 0, 0] }],
      ["leg_fr", 1.0, { rot: [-22, 0, 0] }],
      ["leg_bl", 0.0, { rot: [-22, 0, 0] }],
      ["leg_bl", 0.5, { rot: [22, 0, 0] }],
      ["leg_bl", 1.0, { rot: [-22, 0, 0] }],

      ["tail", 0.0, { rot: [0, 0, -15] }],
      ["tail", 0.5, { rot: [0, 0, 15] }],
      ["tail", 1.0, { rot: [0, 0, -15] }],
    ],
  });
});
