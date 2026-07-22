import { figure } from "../../src/dsl";

// A crooked bog witch with a cutout-torn robe, bent hat, raised hex staff,
// cautious stalking gait, and a separate two-handed spell cast.
export default figure("witch", ({
  asciiTexture,
  bipedWalk,
  box,
  clip,
  defaultClip,
  mat,
  metadata,
  part,
}) => {
  metadata({
    bodyPlans: ["biped"],
    disposition: "hostile",
    groups: ["fantasy", "monster", "humanoid"],
    habitats: ["land", "underground"],
    scale: "medium",
    themes: ["alchemy", "dark-magic", "scary", "swamp", "witchcraft"],
  });

  mat("skin", "#748a55");
  mat("skin_light", "#9eaa6d");
  mat("robe", "#443450");
  mat("robe_dark", "#292536");
  mat("robe_cutout", { color: "#443450", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("leather", "#60432f");
  mat("wood", "#513b2b");
  mat("wood_light", "#80603a");
  mat("magic", "#9ce567");
  mat("void", "#17151c");

  asciiTexture("witch_face", {
    palette: {
      ".": "#748a55",
      "l": "#9eaa6d",
      "d": "#4f643d",
      "e": "#e4cf65",
      "v": "#17151c",
    },
    pixels: [
      "dd......dd",
      "d........d",
      "..ee..ee..",
      "..vv..vv..",
      "....ll....",
      "...llll...",
      "..d.vv.d..",
      "dd......dd",
    ],
  });
  asciiTexture("ragged_robe", {
    palette: {
      ".": "transparent",
      "c": "#443450",
      "d": "#292536",
      "b": "#60432f",
    },
    pixels: [
      "cccccccccccc",
      "ccccdcccdccc",
      "cccdccccdccc",
      "cccccccccccc",
      "cccbccccbccc",
      "cccccccccccc",
      "c.cccc.cccc.",
      ".ccc..ccc.c.",
      "..cc..cc....",
    ],
  });
  asciiTexture("torn_brim", {
    palette: { ".": "transparent", "c": "#292536", "l": "#574964" },
    pixels: [
      "..cccccccc..",
      ".cccccccccc.",
      "cccclllccccc",
      "cccccccccccc",
      ".ccc.cccc.c.",
      "..cc..ccc...",
    ],
  });
  asciiTexture("orb_runes", {
    palette: { ".": "#9ce567", "l": "#d3ff99", "d": "#4d7f3b" },
    pixels: [
      "ddllll",
      "d....l",
      "l.dd.l",
      "l.dd.l",
      "l....d",
      "lllldd",
    ],
  });

  part("pelvis", box({
    at: [0, 0.72, 0.02],
    rot: [0, 0, -3],
    size: [0.62, 0.28, 0.4],
    material: "robe_dark",
  }));
  part("robe_skirt", box({
    parent: "pelvis",
    at: [0, -0.08, 0.02],
    rot: [0, 0, 2],
    size: [0.86, 0.82, 0.52],
    material: "robe_cutout",
    faces: {
      north: { texture: "ragged_robe" },
      south: { texture: "ragged_robe" },
      east: { texture: "ragged_robe" },
      west: { texture: "ragged_robe" },
    },
  }));
  part("torso", box({
    parent: "pelvis",
    at: [0, 0.43, 0.04],
    rot: [9, 0, -3],
    size: [0.68, 0.72, 0.4],
    material: "robe",
    joint: { pivot: [0, -0.32, 0.08], axis: [1, 0, 0] },
  }));
  part("shoulder_wrap", box({
    parent: "torso",
    at: [0, 0.24, 0],
    size: [0.84, 0.2, 0.44],
    material: "robe_dark",
  }));
  part("neck", box({
    parent: "torso",
    at: [0.02, 0.48, -0.02],
    rot: [10, 0, 4],
    size: [0.28, 0.3, 0.28],
    material: "skin",
    joint: { pivot: [0, -0.12, 0.08], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0.03, 0.3, -0.08],
    rot: [3, -5, -2],
    size: [0.62, 0.56, 0.5],
    material: "skin",
    faces: { north: { texture: "witch_face" } },
  }));
  part("nose", box({
    parent: "head",
    at: [0.08, -0.02, -0.34],
    rot: [-8, 0, -4],
    size: [0.18, 0.16, 0.28],
    material: "skin_light",
  }));
  part("chin", box({
    parent: "head",
    at: [-0.03, -0.32, -0.08],
    size: [0.32, 0.14, 0.3],
    material: "skin",
  }));

  part("hat_brim", box({
    parent: "head",
    at: [-0.04, 0.34, 0.02],
    rot: [0, 0, -7],
    size: [1.02, 0.08, 0.7],
    material: "robe_cutout",
    faces: {
      north: { texture: "torn_brim" },
      south: { texture: "torn_brim" },
      east: { texture: "torn_brim" },
      west: { texture: "torn_brim" },
      up: { texture: "torn_brim" },
      down: { texture: "torn_brim" },
    },
  }));
  part("hat_crown", box({
    parent: "hat_brim",
    at: [0.04, 0.21, 0.03],
    rot: [0, 0, 7],
    size: [0.58, 0.38, 0.46],
    material: "robe_dark",
  }));
  part("hat_bend", box({
    parent: "hat_crown",
    at: [-0.08, 0.3, 0.01],
    rot: [0, 0, -18],
    size: [0.42, 0.32, 0.38],
    material: "robe",
  }));
  part("hat_tip", box({
    parent: "hat_bend",
    at: [-0.13, 0.24, 0.02],
    rot: [0, 0, -28],
    size: [0.24, 0.26, 0.26],
    material: "robe_dark",
  }));
  part("hat_band", box({
    parent: "hat_crown",
    at: [0, -0.13, -0.25],
    size: [0.6, 0.1, 0.08],
    material: "leather",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`upper_arm_${side}`, box({
      parent: "shoulder_wrap",
      at: [sign * 0.5, -0.22, 0],
      rot: [-8, 0, sign * -6],
      size: [0.24, 0.5, 0.28],
      material: side === "l" ? "robe" : "robe_dark",
      joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] },
    }));
    part(`forearm_${side}`, box({
      parent: `upper_arm_${side}`,
      at: [0, -0.34, -0.03],
      rot: [-6, 0, sign * 4],
      size: [0.2, 0.3, 0.22],
      material: "skin",
    }));
    part(`hand_${side}`, box({
      parent: `forearm_${side}`,
      at: [0, -0.21, -0.04],
      size: [0.23, 0.16, 0.25],
      material: "skin_light",
    }));
    part(`leg_${side}`, box({
      parent: "pelvis",
      at: [sign * 0.17, -0.36, 0],
      rot: [0, 0, sign * 3],
      size: [0.26, 0.52, 0.3],
      material: "robe_dark",
      joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.31, -0.08],
      rot: [0, sign * -3, 0],
      size: [0.32, 0.14, 0.43],
      material: "leather",
    }));
  }

  part("staff", box({
    parent: "hand_r",
    at: [0.02, 0.52, 0.04],
    rot: [0, 0, -5],
    size: [0.11, 1.18, 0.11],
    material: "wood",
  }));
  part("staff_fork_l", box({
    parent: "staff",
    at: [-0.1, 0.64, 0],
    rot: [0, 0, 28],
    size: [0.1, 0.34, 0.1],
    material: "wood_light",
  }));
  part("staff_fork_r", box({
    parent: "staff",
    at: [0.1, 0.64, 0],
    rot: [0, 0, -28],
    size: [0.1, 0.34, 0.1],
    material: "wood_light",
  }));
  part("hex_orb", box({
    parent: "staff",
    at: [0, 0.68, -0.02],
    rot: [0, 0, 45],
    size: [0.28, 0.28, 0.26],
    material: "magic",
    faces: { north: { texture: "orb_runes" }, south: { texture: "orb_runes" } },
  }));

  bipedWalk("crooked_stalk", {
    label: "Crooked stalk",
    fps: 18,
    duration: 1.28,
    cycleDistance: 0.56,
    loop: true,
    samples: 27,
    armSwingDegrees: 7,
    body: "pelvis",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "neck",
    headSwingDegrees: 5,
    leftArm: "upper_arm_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "upper_arm_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.72,
    swingDegrees: 13,
  });
  clip("hex_cast", {
    label: "Hex cast",
    role: "action",
    nextClip: "crooked_stalk",
    fps: 30,
    loop: false,
    keys: [
      ["torso", 0, { rot: [0, 0, 0] }],
      ["torso", 0.28, { rot: [8, -8, 0] }],
      ["torso", 0.58, { rot: [-12, 10, 0] }],
      ["torso", 0.86, { rot: [-8, 7, 0] }],
      ["torso", 1.28, { rot: [0, 0, 0] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.28, { rot: [-8, -12, 0] }],
      ["neck", 0.58, { rot: [13, 18, -5] }],
      ["neck", 1.28, { rot: [0, 0, 0] }],
      ["upper_arm_l", 0, { rot: [0, 0, 0] }],
      ["upper_arm_l", 0.28, { rot: [25, 0, 8] }],
      ["upper_arm_l", 0.58, { rot: [-78, 0, -14] }],
      ["upper_arm_l", 0.86, { rot: [-62, 0, -10] }],
      ["upper_arm_l", 1.28, { rot: [0, 0, 0] }],
      ["upper_arm_r", 0, { rot: [0, 0, 0] }],
      ["upper_arm_r", 0.28, { rot: [18, 0, -5] }],
      ["upper_arm_r", 0.58, { rot: [-54, 0, 9] }],
      ["upper_arm_r", 0.86, { rot: [-42, 0, 7] }],
      ["upper_arm_r", 1.28, { rot: [0, 0, 0] }],
      ["staff", 0, { rot: [0, 0, 0] }],
      ["staff", 0.58, { rot: [-8, 0, -10] }],
      ["staff", 0.86, { rot: [5, 0, 7] }],
      ["staff", 1.28, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("crooked_stalk");
});
