import { figure } from "../../src/dsl";

// A heavy Egyptian hybrid with a crocodile head, lion shoulders and forelegs,
// and broad hippopotamus hindquarters. The three joins remain fully exposed.
export default figure("ammit", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
  quadrupedWalk,
}) => {
  metadata({
    bodyPlans: ["quadruped"],
    disposition: "neutral",
    groups: ["animal", "fantasy"],
    habitats: ["land", "water"],
    scale: "large",
    themes: ["ammit", "crocodile", "hippopotamus", "hybrid", "lion", "mythic"],
  });

  mat("hippo", "#756a70");
  mat("hippo_light", "#96858b");
  mat("hippo_dark", "#51484d");
  mat("lion", "#b77934");
  mat("lion_light", "#daa45b");
  mat("lion_dark", "#52321f");
  mat("croc", "#53683a");
  mat("croc_light", "#76864f");
  mat("croc_dark", "#2d4229");
  mat("mouth", "#512c31");
  mat("tooth", "#eee4c5");
  mat("claw", "#d3c38f");

  asciiTexture("croc_face", {
    palette: { ".": "#76864f", "e": "#d6b84f", "p": "#17160f", "d": "#2d4229" },
    pixels: [
      "dddddddddd",
      "d.eppppe.d",
      "d.eppppe.d",
      "d........d",
      "dd......dd",
      "dddddddddd",
    ],
  });
  asciiTexture("jaw_side", {
    palette: { "h": "#53683a", "m": "#512c31", "t": "#eee4c5" },
    pixels: ["hhhhhhhhhhhh", "tttttttttttt", "mmmmmmmmmmmm", "tttttttttttt", "hhhhhhhhhhhh"],
  });
  asciiTexture("hippo_folds", {
    palette: { ".": "#756a70", "l": "#96858b", "d": "#51484d" },
    pixels: [
      "............",
      "..llll......",
      "......ddd...",
      "....dd...d..",
      "...d.....d..",
      "....dd...d..",
      "......ddd...",
      "............",
    ],
  });
  asciiTexture("lion_paw", {
    palette: { "p": "#52321f", "c": "#17100c" },
    pixels: ["pppppppp", "pcpccppp", "cccccccc"],
  });

  part("hippo_body", box({
    at: [0, 1.02, 0.1],
    size: [1.34, 0.86, 1.68],
    material: "hippo",
    faces: { east: { texture: "hippo_folds" }, west: { texture: "hippo_folds" } },
  }));
  part("hippo_rump", box({
    parent: "hippo_body",
    at: [0, 0.04, 0.68],
    size: [1.4, 0.8, 0.55],
    material: "hippo_light",
  }));
  part("belly", box({
    parent: "hippo_body",
    at: [0, -0.45, 0.1],
    size: [1.12, 0.12, 1.28],
    material: "hippo_dark",
  }));
  part("lion_shoulders", box({
    parent: "hippo_body",
    at: [0, 0.01, -0.68],
    size: [1.16, 0.78, 0.58],
    material: "lion",
  }));
  part("lion_chest", box({
    parent: "lion_shoulders",
    at: [0, 0.02, -0.31],
    size: [1.1, 0.72, 0.24],
    material: "lion_dark",
  }));
  part("croc_neck", box({
    parent: "lion_shoulders",
    at: [0, 0.08, -0.47],
    size: [0.94, 0.54, 0.48],
    material: "croc",
    joint: { pivot: [0, 0, 0.2], axis: [1, 0, 0] },
  }));
  part("croc_head", box({
    parent: "croc_neck",
    at: [0, 0.04, -0.43],
    size: [0.88, 0.5, 0.58],
    material: "croc_light",
    faces: { north: { texture: "croc_face" } },
  }));
  part("upper_snout", box({
    parent: "croc_head",
    at: [0, -0.04, -0.49],
    size: [0.82, 0.32, 0.48],
    material: "croc_light",
  }));
  part("lower_jaw", box({
    parent: "croc_head",
    at: [0, -0.29, -0.27],
    size: [0.78, 0.14, 0.82],
    material: "mouth",
    faces: { east: { texture: "jaw_side" }, west: { texture: "jaw_side" } },
    joint: { pivot: [0, 0.04, 0.36], axis: [1, 0, 0] },
  }));
  for (const [side, x] of [["l", -0.32], ["r", 0.32]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "croc_head",
      at: [x, 0.3, -0.08],
      size: [0.22, 0.19, 0.24],
      material: "croc_dark",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.39, -0.5],
    ["fr", 0.39, -0.5],
  ] as const) {
    part(`lion_leg_${suffix}`, box({
      parent: "hippo_body",
      at: [x, -0.58, z],
      size: [0.25, 0.6, 0.27],
      material: "lion",
      joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] },
    }));
    part(`lion_paw_${suffix}`, box({
      parent: `lion_leg_${suffix}`,
      at: [0, -0.37, -0.06],
      size: [0.34, 0.14, 0.42],
      material: "lion_dark",
      faces: { north: { texture: "lion_paw" } },
    }));
  }
  for (const [suffix, x, z] of [
    ["bl", -0.48, 0.52],
    ["br", 0.48, 0.52],
  ] as const) {
    part(`hippo_leg_${suffix}`, box({
      parent: "hippo_body",
      at: [x, -0.59, z],
      size: [0.34, 0.46, 0.36],
      material: "hippo",
      joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] },
    }));
    part(`hippo_foot_${suffix}`, box({
      parent: `hippo_leg_${suffix}`,
      at: [0, -0.3, -0.05],
      size: [0.44, 0.16, 0.48],
      material: "hippo_dark",
    }));
    for (const [toeIndex, toeX] of [-0.11, 0.11].entries()) {
      part(`toe_${suffix}_${toeIndex + 1}`, box({
        parent: `hippo_foot_${suffix}`,
        at: [toeX, -0.01, -0.25],
        size: [0.09, 0.07, 0.025],
        material: "claw",
      }));
    }
  }

  part("hippo_tail", box({
    parent: "hippo_body",
    at: [0, 0.06, 0.92],
    rot: [-18, 0, 0],
    size: [0.12, 0.36, 0.12],
    material: "hippo_dark",
    joint: { pivot: [0, 0.17, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "hippo_tail",
    at: [0, -0.23, 0.02],
    size: [0.18, 0.14, 0.16],
    material: "hippo_dark",
  }));

  quadrupedWalk("devourer_stalk", {
    label: "Devourer stalk",
    fps: 20,
    duration: 1.5,
    cycleDistance: 0.64,
    gait: "walk",
    loop: true,
    samples: 25,
    contactParts: {
      frontLeft: "lion_paw_fl",
      frontRight: "lion_paw_fr",
      backLeft: "hippo_foot_bl",
      backRight: "hippo_foot_br",
    },
    body: "hippo_body",
    bodyBob: 0.015,
    bodyBobCenter: 0.017,
    head: "croc_neck",
    headSwingDegrees: 1.8,
    legs: {
      frontLeft: "lion_leg_fl",
      frontRight: "lion_leg_fr",
      backLeft: "hippo_leg_bl",
      backRight: "hippo_leg_br",
    },
    stanceRatio: 0.76,
    swingDegrees: 13,
    tail: "hippo_tail",
    tailSwingDegrees: 5,
    tracks: [
      followThrough("croc_head", { source: "hippo_body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.13 }),
    ],
  });
  clip("crushing_bite", {
    label: "Crushing bite",
    role: "action",
    nextClip: "devourer_stalk",
    fps: 30,
    loop: false,
    keys: [
      ["croc_neck", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["croc_neck", 0.28, { at: [0, 0.02, 0.08], rot: [-8, 0, 0] }],
      ["croc_neck", 0.62, { at: [0, -0.05, -0.08], rot: [10, 0, 0] }],
      ["croc_neck", 0.84, { at: [0, -0.02, -0.03], rot: [4, 0, 0] }],
      ["croc_neck", 1.3, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["lower_jaw", 0, { rot: [0, 0, 0] }],
      ["lower_jaw", 0.28, { rot: [-30, 0, 0] }],
      ["lower_jaw", 0.62, { rot: [-36, 0, 0] }],
      ["lower_jaw", 0.7, { rot: [2, 0, 0] }],
      ["lower_jaw", 0.84, { rot: [-7, 0, 0] }],
      ["lower_jaw", 1.3, { rot: [0, 0, 0] }],
      ["hippo_body", 0, { at: [0, 0, 0] }],
      ["hippo_body", 0.28, { at: [0, -0.02, 0.02] }],
      ["hippo_body", 0.62, { at: [0, 0.01, -0.03] }],
      ["hippo_body", 1.3, { at: [0, 0, 0] }],
    ],
  });
  defaultClip("devourer_stalk");
});
