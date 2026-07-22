import { figure } from "../../src/dsl";

// A classic humanoid mandrake: knotted root body, twig arms, root feet,
// ragged leaf cards, a waddling gait, and a separate full-bodied scream.
export default figure("mandrake", ({
  asciiTexture,
  bipedWalk,
  box,
  clip,
  defaultClip,
  mat,
  metadata,
  part,
  plane,
  swing,
}) => {
  metadata({
    bodyPlans: ["biped", "rooted"],
    disposition: "neutral",
    groups: ["plant", "fantasy"],
    habitats: ["land", "underground"],
    scale: "small",
    themes: ["folklore", "living-growth", "mandrake", "mobile", "woodland"],
  });

  mat("root", "#9a7b52");
  mat("root_light", "#c2a36e");
  mat("root_dark", "#5e4b38");
  mat("leaf", { color: "#4f7b3f", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("leaf_light", { color: "#78a653", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("mouth", "#35232a");
  mat("gum", "#8a4651");

  asciiTexture("root_face", {
    palette: {
      ".": "#9a7b52",
      "l": "#c2a36e",
      "d": "#5e4b38",
      "e": "#29251f",
    },
    pixels: [
      "dd........dd",
      "d..ll..ll..d",
      "...ee..ee...",
      "...ee..ee...",
      ".....dd.....",
      "..d......d..",
      ".d........d.",
      "dd........dd",
    ],
  });
  asciiTexture("root_rings", {
    palette: { ".": "#9a7b52", "l": "#c2a36e", "d": "#5e4b38" },
    pixels: ["dddddddd", "d.ll.l.d", ".l....l.", "..d..d..", "dddddddd"],
  });
  asciiTexture("mandrake_leaf", {
    palette: {
      ".": "transparent",
      "g": "#4f7b3f",
      "l": "#78a653",
      "d": "#315132",
    },
    pixels: [
      "....gg....",
      "..ggllgg..",
      ".ggllllgg.",
      "gggllllggg",
      ".gggllggg.",
      "..ggllgg..",
      "...gllg...",
      "....gg....",
    ],
  });

  part("body", box({
    at: [0, 0.77, 0],
    size: [0.66, 0.74, 0.48],
    material: "root",
    faces: {
      north: { texture: "root_face" },
      east: { texture: "root_rings" },
      west: { texture: "root_rings" },
    },
  }));
  part("brow", box({
    parent: "body",
    at: [0, 0.16, -0.27],
    rot: [0, 0, -2],
    size: [0.48, 0.1, 0.08],
    material: "root_dark",
  }));
  part("mouth_cavity", box({
    parent: "body",
    at: [0, -0.12, -0.28],
    size: [0.3, 0.16, 0.1],
    material: "mouth",
  }));
  part("lower_lip", box({
    parent: "mouth_cavity",
    at: [0, -0.1, -0.02],
    size: [0.26, 0.1, 0.1],
    material: "gum",
    joint: { pivot: [0, 0.04, 0.04], axis: [1, 0, 0] },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`arm_${side}`, box({
      parent: "body",
      at: [sign * 0.39, 0.01, 0],
      rot: [0, 0, sign * -12],
      size: [0.2, 0.58, 0.2],
      material: "root",
      joint: { pivot: [0, 0.26, 0], axis: [1, 0, 0] },
    }));
    part(`hand_${side}`, box({
      parent: `arm_${side}`,
      at: [sign * 0.04, -0.34, -0.01],
      rot: [0, sign * 7, sign * 8],
      size: [0.24, 0.2, 0.22],
      material: "root_dark",
    }));
    part(`leg_${side}`, box({
      parent: "body",
      at: [sign * 0.19, -0.48, 0.03],
      size: [0.25, 0.4, 0.28],
      material: "root_dark",
      joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [sign * 0.03, -0.25, -0.09],
      size: [0.34, 0.12, 0.42],
      material: "root",
      faces: { up: { texture: "root_rings" } },
    }));
  }

  part("crown", box({
    parent: "body",
    at: [0, 0.48, 0.02],
    size: [0.24, 0.34, 0.24],
    material: "root_light",
  }));
  for (const leaf of [
    { name: "center", at: [0, 0.32, -0.01], rot: [0, 0, 0], size: [0.34, 0.58], material: "leaf_light" },
    { name: "l", at: [-0.18, 0.26, 0], rot: [0, -8, 34], size: [0.32, 0.54], material: "leaf" },
    { name: "r", at: [0.18, 0.27, 0.01], rot: [0, 9, -32], size: [0.32, 0.54], material: "leaf_light" },
    { name: "back_l", at: [-0.11, 0.23, 0.07], rot: [0, 22, 17], size: [0.28, 0.48], material: "leaf" },
    { name: "back_r", at: [0.12, 0.22, 0.08], rot: [0, -23, -18], size: [0.28, 0.48], material: "leaf" },
  ] as const) {
    part(`leaf_${leaf.name}`, plane({
      parent: "crown",
      at: leaf.at,
      rot: leaf.rot,
      size: leaf.size,
      sidedness: "double",
      material: leaf.material,
      texture: "mandrake_leaf",
      joint: { pivot: [0, -leaf.size[1] * 0.42, 0], axis: [0, 0, 1] },
    }));
  }

  bipedWalk("root_waddle", {
    label: "Root waddle",
    fps: 20,
    duration: 1.04,
    cycleDistance: 0.44,
    loop: true,
    samples: 23,
    armSwingDegrees: 9,
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.016,
    leftArm: "arm_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "arm_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.7,
    swingDegrees: 17,
    tracks: [
      swing("leaf_center", { axis: "z", degrees: 3, phase: 0.08 }),
      swing("leaf_l", { axis: "z", degrees: 5, phase: 0.32 }),
      swing("leaf_r", { axis: "z", degrees: 5, phase: 0.68 }),
    ],
  });
  clip("mandrake_scream", {
    label: "Mandrake scream",
    role: "action",
    nextClip: "root_waddle",
    fps: 30,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["body", 0.2, { at: [0, -0.03, 0], scale: [1.06, 0.92, 1.04] }],
      ["body", 0.42, { at: [0, 0.08, 0], scale: [0.96, 1.12, 0.98] }],
      ["body", 0.82, { at: [0, 0.06, 0], scale: [0.98, 1.08, 1] }],
      ["body", 1.26, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["lower_lip", 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["lower_lip", 0.2, { at: [0, 0.02, 0], scale: [0.9, 0.6, 1] }],
      ["lower_lip", 0.42, { at: [0, -0.12, 0], scale: [1.2, 2.1, 1] }],
      ["lower_lip", 0.82, { at: [0, -0.09, 0], scale: [1.12, 1.7, 1] }],
      ["lower_lip", 1.26, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["arm_l", 0, { rot: [0, 0, 0] }],
      ["arm_l", 0.42, { rot: [0, 0, 62] }],
      ["arm_l", 0.82, { rot: [0, 0, 48] }],
      ["arm_l", 1.26, { rot: [0, 0, 0] }],
      ["arm_r", 0, { rot: [0, 0, 0] }],
      ["arm_r", 0.42, { rot: [0, 0, -62] }],
      ["arm_r", 0.82, { rot: [0, 0, -48] }],
      ["arm_r", 1.26, { rot: [0, 0, 0] }],
      ["leaf_l", 0, { rot: [0, 0, 0] }],
      ["leaf_l", 0.42, { rot: [0, 0, 18] }],
      ["leaf_l", 0.58, { rot: [0, 0, -16] }],
      ["leaf_l", 0.82, { rot: [0, 0, 14] }],
      ["leaf_l", 1.26, { rot: [0, 0, 0] }],
      ["leaf_r", 0, { rot: [0, 0, 0] }],
      ["leaf_r", 0.42, { rot: [0, 0, -17] }],
      ["leaf_r", 0.58, { rot: [0, 0, 15] }],
      ["leaf_r", 0.82, { rot: [0, 0, -13] }],
      ["leaf_r", 1.26, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("root_waddle");
});
