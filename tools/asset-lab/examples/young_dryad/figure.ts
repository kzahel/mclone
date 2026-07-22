import { figure } from "../../src/dsl";

// A young dryad whose humanoid shape is grown rather than dressed: root feet,
// sapling legs, bark torso and face, branch arms, twig hands, and a small crown
// of fixed double-sided leaves.
export default figure("young_dryad", ({
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
    disposition: "passive",
    groups: ["plant", "fantasy"],
    habitats: ["land"],
    scale: "medium",
    themes: ["dryad", "living-growth", "sapling", "woodland", "young"],
  });

  mat("bark", "#76583b");
  mat("bark_light", "#a07a4e");
  mat("bark_dark", "#46392d");
  mat("root", "#5d4935");
  mat("sap", "#bdcf70");
  mat("leaf", { color: "#46733e", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("leaf_light", { color: "#6f9d50", alphaMode: "mask", alphaCutoff: 0.1 });

  asciiTexture("dryad_face", {
    palette: { ".": "#76583b", "l": "#a07a4e", "d": "#46392d", "e": "#bdcf70" },
    pixels: [
      "dd........dd",
      "d..ll..ll..d",
      "...ee..ee...",
      "...ee..ee...",
      ".....dd.....",
      "..d......d..",
      "...dddddd...",
      "dd........dd",
    ],
  });
  asciiTexture("bark_grain", {
    palette: { ".": "#76583b", "l": "#a07a4e", "d": "#46392d", "s": "#bdcf70" },
    pixels: [
      "d..l..d.",
      ".l....l.",
      "..d..d..",
      ".s....l.",
      "d..l..d.",
      "llllllll",
    ],
  });
  asciiTexture("root_rings", {
    palette: { ".": "#5d4935", "l": "#76583b", "d": "#46392d" },
    pixels: ["dddddddd", "d.ll.l.d", ".l....l.", "..d..d..", "dddddddd"],
  });
  asciiTexture("dryad_leaf", {
    palette: { ".": "transparent", "g": "#46733e", "l": "#6f9d50", "d": "#2e5332" },
    pixels: [
      "...gg...",
      "..gllg..",
      ".gllllg.",
      "ggllllgg",
      ".ggllgg.",
      "..gllg..",
      "...gg...",
      "...gg...",
    ],
  });

  part("root_pelvis", box({
    at: [0, 0.58, 0],
    size: [0.68, 0.38, 0.56],
    material: "root",
    faces: { east: { texture: "root_rings" }, west: { texture: "root_rings" } },
  }));
  part("trunk", box({
    parent: "root_pelvis",
    at: [0, 0.65, 0],
    size: [0.58, 1.02, 0.48],
    material: "bark",
    faces: { north: { texture: "bark_grain" }, east: { texture: "bark_grain" }, west: { texture: "bark_grain" } },
  }));
  part("heartwood", box({
    parent: "trunk",
    at: [0, -0.04, -0.28],
    size: [0.34, 0.46, 0.1],
    material: "bark_dark",
    faces: { north: { texture: "root_rings" } },
  }));
  part("head", box({
    parent: "trunk",
    at: [0, 0.72, -0.02],
    size: [0.56, 0.5, 0.46],
    material: "bark_light",
    faces: { north: { texture: "dryad_face" } },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`branch_arm_${side}`, box({
      parent: "trunk",
      at: [sign * 0.43, 0.2, 0],
      rot: [0, sign * -4, sign * -10],
      size: [0.56, 0.18, 0.2],
      material: "bark",
      faces: { north: { texture: "bark_grain" }, south: { texture: "bark_grain" } },
      joint: { pivot: [sign * -0.24, 0, 0], axis: [1, 0, 0] },
    }));
    part(`branch_forearm_${side}`, box({
      parent: `branch_arm_${side}`,
      at: [sign * 0.5, -0.04, 0],
      rot: [0, 0, sign * -12],
      size: [0.46, 0.15, 0.17],
      material: "bark_light",
    }));
    part(`twig_hand_${side}`, box({
      parent: `branch_forearm_${side}`,
      at: [sign * 0.28, -0.02, 0],
      rot: [0, sign * -5, sign * -8],
      size: [0.2, 0.2, 0.2],
      material: "bark_dark",
    }));
    part(`root_leg_${side}`, box({
      parent: "root_pelvis",
      at: [sign * 0.2, -0.34, 0.02],
      size: [0.26, 0.4, 0.28],
      material: "bark_dark",
      joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] },
    }));
    part(`root_foot_${side}`, box({
      parent: `root_leg_${side}`,
      at: [sign * 0.04, -0.34, -0.09],
      size: [0.38, 0.16, 0.44],
      material: "root",
      faces: { up: { texture: "root_rings" } },
    }));
  }

  part("crown", box({
    parent: "head",
    at: [0, 0.34, 0.03],
    size: [0.28, 0.28, 0.22],
    material: "bark_dark",
  }));
  for (const leaf of [
    { name: "center", at: [0, 0.3, -0.08], rot: [0, 0, 0], size: [0.3, 0.48], material: "leaf_light" },
    { name: "l", at: [-0.18, 0.24, -0.03], rot: [0, -7, 31], size: [0.3, 0.44], material: "leaf" },
    { name: "r", at: [0.18, 0.24, 0.03], rot: [0, 8, -31], size: [0.3, 0.44], material: "leaf_light" },
    { name: "back_l", at: [-0.1, 0.22, 0.08], rot: [0, 20, 16], size: [0.26, 0.4], material: "leaf" },
    { name: "back_r", at: [0.11, 0.21, 0.1], rot: [0, -22, -17], size: [0.26, 0.4], material: "leaf" },
  ] as const) {
    part(`leaf_${leaf.name}`, plane({
      parent: "crown",
      at: leaf.at,
      rot: leaf.rot,
      size: leaf.size,
      sidedness: "double",
      material: leaf.material,
      texture: "dryad_leaf",
      joint: { pivot: [0, -leaf.size[1] * 0.4, 0], axis: [0, 0, 1] },
    }));
  }

  bipedWalk("root_walk", {
    label: "Root walk",
    fps: 20,
    duration: 1.14,
    cycleDistance: 0.46,
    loop: true,
    samples: 23,
    armSwingDegrees: 8,
    body: "root_pelvis",
    bodyBob: 0.013,
    bodyBobCenter: 0.015,
    head: "head",
    headSwingDegrees: 2.5,
    leftArm: "branch_arm_l",
    leftContact: "root_foot_l",
    leftLeg: "root_leg_l",
    rightArm: "branch_arm_r",
    rightContact: "root_foot_r",
    rightLeg: "root_leg_r",
    stanceRatio: 0.72,
    swingDegrees: 16,
    tracks: [
      swing("trunk", { axis: "z", degrees: 1.8, phase: 0.24 }),
      swing("leaf_center", { axis: "z", degrees: 2.5, phase: 0.08 }),
      swing("leaf_l", { axis: "z", degrees: 4, phase: 0.33 }),
      swing("leaf_r", { axis: "z", degrees: -4, phase: 0.67 }),
    ],
  });
  clip("light_unfurl", {
    label: "Light unfurl",
    role: "action",
    nextClip: "root_walk",
    fps: 30,
    loop: false,
    keys: [
      ["trunk", 0, { rot: [0, 0, 0] }],
      ["trunk", 0.34, { rot: [-3, 0, -3] }],
      ["trunk", 0.72, { rot: [-5, 0, 2] }],
      ["trunk", 1.3, { rot: [0, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.34, { rot: [-7, -7, 0] }],
      ["head", 0.72, { rot: [-10, 6, 0] }],
      ["head", 1.3, { rot: [0, 0, 0] }],
      ["branch_arm_l", 0, { rot: [0, 0, 0] }],
      ["branch_arm_l", 0.34, { rot: [0, 0, -32] }],
      ["branch_arm_l", 0.72, { rot: [0, 0, -48] }],
      ["branch_arm_l", 1.3, { rot: [0, 0, 0] }],
      ["branch_arm_r", 0, { rot: [0, 0, 0] }],
      ["branch_arm_r", 0.34, { rot: [0, 0, 32] }],
      ["branch_arm_r", 0.72, { rot: [0, 0, 48] }],
      ["branch_arm_r", 1.3, { rot: [0, 0, 0] }],
      ["leaf_center", 0, { rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["leaf_center", 0.34, { rot: [0, 0, -4], scale: [1.08, 1.08, 1] }],
      ["leaf_center", 0.72, { rot: [0, 0, 5], scale: [1.16, 1.14, 1] }],
      ["leaf_center", 1.3, { rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["leaf_l", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["leaf_l", 0.34, { at: [-0.03, 0.02, 0], rot: [0, 0, 10] }],
      ["leaf_l", 0.72, { at: [-0.06, 0.04, 0], rot: [0, 0, 18] }],
      ["leaf_l", 1.3, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["leaf_r", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["leaf_r", 0.34, { at: [0.03, 0.02, 0], rot: [0, 0, -10] }],
      ["leaf_r", 0.72, { at: [0.06, 0.04, 0], rot: [0, 0, -18] }],
      ["leaf_r", 1.3, { at: [0, 0, 0], rot: [0, 0, 0] }],
    ],
  });
  defaultClip("root_walk");
});
