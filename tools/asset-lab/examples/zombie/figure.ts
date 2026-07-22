import { figure } from "../../src/dsl";

// A lopsided graveyard zombie with a loose jaw, mismatched limbs, a dragging
// shamble, and a separate hungry lunge.
export default figure("zombie", ({ asciiTexture, bipedWalk, box, clip, defaultClip, mat, metadata, part }) => {
  metadata({
    bodyPlans: ["biped"],
    disposition: "hostile",
    groups: ["fantasy", "monster", "humanoid"],
    habitats: ["land", "underground"],
    scale: "medium",
    themes: ["decay", "graveyard", "scary", "undead"],
  });
  mat("skin", "#71815b");
  mat("skin_light", "#91a06e");
  mat("bruise", "#51553f");
  mat("shirt", "#596c64");
  mat("shirt_dark", "#344840");
  mat("trousers", "#3e4655");
  mat("boot", "#282725");
  mat("bone", "#d6cfac");
  mat("void", "#171b16");

  asciiTexture("dead_face", {
    palette: {
      ".": "#71815b",
      "l": "#91a06e",
      "d": "#51553f",
      "v": "#171b16",
      "t": "#d6cfac",
    },
    pixels: [
      "lldddlll",
      "l......d",
      ".vv..vvd",
      ".vv..vvd",
      "...dd...",
      "..vttv..",
      ".v....v.",
      "dd....dd",
    ],
  });
  asciiTexture("torn_shirt", {
    palette: { ".": "#596c64", "l": "#718079", "d": "#344840", "b": "#592f2d" },
    pixels: [
      "lll....lll",
      "l........l",
      "..d....d..",
      "...bbbb...",
      "..bb......",
      "....d.....",
      ".d......d.",
      "dd......dd",
    ],
  });

  part("pelvis", box({
    at: [0, 0.68, 0],
    size: [0.62, 0.24, 0.38],
    material: "trousers",
  }));
  part("torso", box({
    parent: "pelvis",
    at: [0, 0.38, 0],
    rot: [4, 0, -3],
    size: [0.72, 0.76, 0.4],
    material: "shirt",
    faces: { north: { texture: "torn_shirt" } },
    joint: { pivot: [0, -0.33, 0.08], axis: [1, 0, 0] },
  }));
  part("shirt_hem", box({
    parent: "torso",
    at: [0.12, -0.41, -0.03],
    rot: [0, 0, -4],
    size: [0.48, 0.16, 0.42],
    material: "shirt_dark",
  }));
  part("neck", box({
    parent: "torso",
    at: [0.04, 0.48, -0.01],
    rot: [7, 0, 6],
    size: [0.3, 0.28, 0.3],
    material: "bruise",
    joint: { pivot: [0, -0.12, 0.08], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0.03, 0.32, -0.07],
    rot: [5, -5, -4],
    size: [0.66, 0.58, 0.54],
    material: "skin",
    faces: { north: { texture: "dead_face" } },
  }));
  part("jaw", box({
    parent: "head",
    at: [0.08, -0.34, -0.06],
    rot: [-7, 0, 4],
    size: [0.4, 0.18, 0.38],
    material: "bruise",
    joint: { pivot: [0, 0.07, 0.14], axis: [1, 0, 0] },
  }));
  part("hair", box({
    parent: "head",
    at: [-0.08, 0.3, 0.05],
    rot: [0, 0, -5],
    size: [0.52, 0.12, 0.46],
    material: "void",
  }));

  part("upper_arm_l", box({
    parent: "torso",
    at: [-0.46, 0.16, 0],
    rot: [-58, 0, -7],
    size: [0.26, 0.6, 0.28],
    material: "shirt_dark",
    joint: { pivot: [0, 0.27, 0], axis: [1, 0, 0] },
  }));
  part("forearm_l", box({
    parent: "upper_arm_l",
    at: [0, -0.38, -0.02],
    rot: [-10, 0, -4],
    size: [0.23, 0.4, 0.24],
    material: "skin",
  }));
  part("hand_l", box({
    parent: "forearm_l",
    at: [0, -0.25, -0.05],
    size: [0.27, 0.16, 0.3],
    material: "skin_light",
  }));
  part("upper_arm_r", box({
    parent: "torso",
    at: [0.46, 0.13, 0.01],
    rot: [-43, 0, 9],
    size: [0.28, 0.54, 0.29],
    material: "shirt",
    joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
  }));
  part("forearm_r", box({
    parent: "upper_arm_r",
    at: [0.02, -0.35, -0.03],
    rot: [12, 0, 6],
    size: [0.22, 0.38, 0.23],
    material: "bone",
  }));
  part("hand_r", box({
    parent: "forearm_r",
    at: [0, -0.24, -0.05],
    size: [0.25, 0.15, 0.29],
    material: "skin",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`leg_${side}`, box({
      parent: "pelvis",
      at: [sign * 0.17, -0.3, 0],
      rot: side === "r" ? [0, 0, 5] : [0, 0, -2],
      size: [side === "r" ? 0.27 : 0.3, 0.5, 0.31],
      material: side === "r" ? "shirt_dark" : "trousers",
      joint: { pivot: [0, 0.24, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.31, -0.08],
      rot: side === "r" ? [0, 8, 0] : [0, -3, 0],
      size: [0.34, 0.14, side === "r" ? 0.45 : 0.4],
      material: "boot",
    }));
  }

  bipedWalk("dragging_shamble", {
    label: "Dragging shamble",
    fps: 18,
    duration: 1.36,
    cycleDistance: 0.54,
    loop: true,
    samples: 29,
    armSwingDegrees: 7,
    body: "pelvis",
    bodyBob: 0.018,
    bodyBobCenter: 0.02,
    head: "neck",
    headSwingDegrees: 6,
    leftArm: "upper_arm_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "upper_arm_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.72,
    swingDegrees: 14,
  });
  clip("hungry_lunge", {
    label: "Hungry lunge",
    role: "action",
    nextClip: "dragging_shamble",
    fps: 30,
    loop: false,
    keys: [
      ["torso", 0, { rot: [0, 0, 0] }],
      ["torso", 0.26, { rot: [10, 0, 0] }],
      ["torso", 0.48, { rot: [-24, 0, 0] }],
      ["torso", 0.72, { rot: [-17, 0, 0] }],
      ["torso", 1.14, { rot: [0, 0, 0] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.26, { rot: [-12, 0, 0] }],
      ["neck", 0.48, { rot: [18, 0, -7] }],
      ["neck", 1.14, { rot: [0, 0, 0] }],
      ["jaw", 0, { rot: [0, 0, 0] }],
      ["jaw", 0.48, { rot: [-31, 0, 0] }],
      ["jaw", 0.8, { rot: [-20, 0, 0] }],
      ["jaw", 1.14, { rot: [0, 0, 0] }],
      ["upper_arm_l", 0, { rot: [0, 0, 0] }],
      ["upper_arm_l", 0.48, { rot: [-28, 0, -8] }],
      ["upper_arm_l", 1.14, { rot: [0, 0, 0] }],
      ["upper_arm_r", 0, { rot: [0, 0, 0] }],
      ["upper_arm_r", 0.48, { rot: [-35, 0, 10] }],
      ["upper_arm_r", 1.14, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("dragging_shamble");
});
