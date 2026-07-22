import { figure } from "../../src/dsl";

// A tall, hunched werewolf with digitigrade legs, oversized claws, a high
// tail, predatory stalking gait, and a separate moon-howl action.
export default figure("werewolf", ({
  asciiTexture,
  bipedWalk,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
}) => {
  metadata({
    bodyPlans: ["biped"],
    disposition: "hostile",
    groups: ["animal", "fantasy", "monster", "humanoid"],
    habitats: ["land"],
    scale: "large",
    themes: ["curse", "lycanthrope", "moonlit", "nocturnal", "scary"],
  });

  mat("fur", "#4f4841");
  mat("fur_light", "#756a5e");
  mat("fur_dark", "#2b2927");
  mat("muzzle", "#877868");
  mat("nose", "#151516");
  mat("eye", "#d9b73f");
  mat("mouth", "#542c32");
  mat("tooth", "#e8dfc5");
  mat("claw", "#d4c59d");

  asciiTexture("wolf_face", {
    palette: {
      ".": "#4f4841",
      "l": "#756a5e",
      "d": "#2b2927",
      "e": "#d9b73f",
      "v": "#151516",
    },
    pixels: [
      "dd......dd",
      "d.ll..ll.d",
      ".ee....ee.",
      ".ev....ve.",
      "...dddd...",
      "..d....d..",
      ".d......d.",
      "dd......dd",
    ],
  });
  asciiTexture("snarl", {
    palette: { ".": "#877868", "n": "#151516", "m": "#542c32", "t": "#e8dfc5" },
    pixels: [
      "..nnnnnn..",
      ".nn....nn.",
      "...mmmm...",
      "..ttmmtt..",
      ".tmmmmmmmt",
      "..t.t..t..",
    ],
  });
  asciiTexture("chest_fur", {
    palette: { ".": "#4f4841", "l": "#756a5e", "d": "#2b2927" },
    pixels: [
      "ddlllllllldd",
      "d..llllll..d",
      "...llllll...",
      "....llll....",
      "d....ll....d",
      "dd........dd",
    ],
  });
  asciiTexture("claw_marks", {
    palette: { ".": "#2b2927", "c": "#d4c59d" },
    pixels: [
      "..........",
      ".c..c..c..",
      "cc.cc.cccc",
    ],
  });

  part("pelvis", box({
    at: [0, 1.0, 0.12],
    rot: [5, 0, 0],
    size: [0.72, 0.34, 0.5],
    material: "fur_dark",
  }));
  part("torso", box({
    parent: "pelvis",
    at: [0, 0.46, -0.05],
    rot: [14, 0, 0],
    size: [0.88, 0.78, 0.5],
    material: "fur",
    faces: { north: { texture: "chest_fur" } },
    joint: { pivot: [0, -0.34, 0.1], axis: [1, 0, 0] },
  }));
  part("shoulders", box({
    parent: "torso",
    at: [0, 0.28, -0.02],
    size: [1.06, 0.26, 0.56],
    material: "fur_dark",
  }));
  part("back_hackle", box({
    parent: "torso",
    at: [0, 0.08, 0.3],
    rot: [-8, 0, 0],
    size: [0.48, 0.54, 0.18],
    material: "fur_light",
  }));
  part("neck", box({
    parent: "torso",
    at: [0, 0.54, -0.11],
    rot: [4, 0, 0],
    size: [0.42, 0.38, 0.4],
    material: "fur_dark",
    joint: { pivot: [0, -0.16, 0.12], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.28, -0.2],
    rot: [5, 0, 0],
    size: [0.72, 0.58, 0.58],
    material: "fur",
    faces: { north: { texture: "wolf_face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.4],
    rot: [-4, 0, 0],
    size: [0.46, 0.3, 0.34],
    material: "muzzle",
    faces: { north: { texture: "snarl" } },
  }));
  part("lower_jaw", box({
    parent: "muzzle",
    at: [0, -0.22, 0.02],
    size: [0.42, 0.14, 0.3],
    material: "mouth",
    joint: { pivot: [0, 0.06, 0.12], axis: [1, 0, 0] },
  }));
  part("nose", box({
    parent: "muzzle",
    at: [0, 0.02, -0.2],
    size: [0.28, 0.16, 0.12],
    material: "nose",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.25, 0.38, 0.02],
      rot: [-5, 0, sign * -15],
      size: [0.2, 0.38, 0.16],
      material: "fur_dark",
      joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
    }));
    part(`upper_arm_${side}`, box({
      parent: "shoulders",
      at: [sign * 0.59, -0.3, -0.02],
      rot: [-10, 0, sign * -7],
      size: [0.3, 0.62, 0.34],
      material: "fur",
      joint: { pivot: [0, 0.28, 0], axis: [1, 0, 0] },
    }));
    part(`forearm_${side}`, box({
      parent: `upper_arm_${side}`,
      at: [0, -0.44, -0.06],
      rot: [-13, 0, sign * 4],
      size: [0.28, 0.46, 0.31],
      material: "fur_dark",
    }));
    part(`hand_${side}`, box({
      parent: `forearm_${side}`,
      at: [0, -0.31, -0.09],
      size: [0.36, 0.19, 0.42],
      material: "fur_dark",
      faces: { north: { texture: "claw_marks" } },
    }));
    part(`thigh_${side}`, box({
      parent: "pelvis",
      at: [sign * 0.22, -0.37, 0],
      rot: [-8, 0, sign * 3],
      size: [0.34, 0.54, 0.4],
      material: "fur",
      joint: { pivot: [0, 0.26, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${side}`, box({
      parent: `thigh_${side}`,
      at: [0, -0.39, 0.08],
      rot: [17, 0, sign * -2],
      size: [0.27, 0.42, 0.3],
      material: "fur_dark",
    }));
    part(`foot_${side}`, box({
      parent: `shin_${side}`,
      at: [0, -0.27, -0.12],
      rot: [-8, sign * -3, 0],
      size: [0.38, 0.16, 0.52],
      material: "fur_dark",
      faces: { north: { texture: "claw_marks" } },
    }));
  }

  part("tail_root", box({
    parent: "pelvis",
    at: [0, 0.02, 0.38],
    rot: [-55, 0, 0],
    size: [0.3, 0.62, 0.3],
    material: "fur_dark",
    joint: { pivot: [0, -0.28, 0], axis: [0, 1, 0] },
  }));
  part("tail_mid", box({
    parent: "tail_root",
    at: [0, 0.45, 0.02],
    rot: [-14, 0, 0],
    size: [0.25, 0.42, 0.25],
    material: "fur",
  }));
  part("tail_tip", box({
    parent: "tail_mid",
    at: [0, 0.32, 0],
    rot: [-12, 0, 0],
    size: [0.19, 0.3, 0.19],
    material: "fur_light",
  }));

  bipedWalk("predator_stalk", {
    label: "Predator stalk",
    fps: 20,
    duration: 1.06,
    cycleDistance: 0.7,
    loop: true,
    samples: 23,
    armSwingDegrees: 13,
    body: "pelvis",
    bodyBob: 0.018,
    bodyBobCenter: 0.02,
    head: "neck",
    headSwingDegrees: 4,
    leftArm: "upper_arm_l",
    leftContact: "foot_l",
    leftLeg: "thigh_l",
    rightArm: "upper_arm_r",
    rightContact: "foot_r",
    rightLeg: "thigh_r",
    stanceRatio: 0.68,
    swingDegrees: 18,
    tracks: [
      followThrough("tail_root", {
        source: "pelvis",
        sourceChannel: "pos",
        sourceAxis: "y",
        axis: "y",
        degrees: 10,
        overshoot: 0.5,
        lag: 0.14,
      }),
    ],
  });
  clip("moon_howl", {
    label: "Moon howl",
    role: "action",
    nextClip: "predator_stalk",
    fps: 30,
    loop: false,
    keys: [
      ["torso", 0, { rot: [0, 0, 0] }],
      ["torso", 0.3, { rot: [13, 0, 0] }],
      ["torso", 0.62, { rot: [-16, 0, 0] }],
      ["torso", 1.16, { rot: [-13, 0, 0] }],
      ["torso", 1.52, { rot: [0, 0, 0] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.3, { rot: [0, 0, 0] }],
      ["neck", 0.62, { rot: [38, 0, 0] }],
      ["neck", 1.16, { rot: [34, 0, 0] }],
      ["neck", 1.52, { rot: [0, 0, 0] }],
      ["lower_jaw", 0, { rot: [0, 0, 0] }],
      ["lower_jaw", 0.62, { rot: [-28, 0, 0] }],
      ["lower_jaw", 1.16, { rot: [-25, 0, 0] }],
      ["lower_jaw", 1.52, { rot: [0, 0, 0] }],
      ["upper_arm_l", 0, { rot: [0, 0, 0] }],
      ["upper_arm_l", 0.62, { rot: [22, 0, -28] }],
      ["upper_arm_l", 1.16, { rot: [18, 0, -24] }],
      ["upper_arm_l", 1.52, { rot: [0, 0, 0] }],
      ["upper_arm_r", 0, { rot: [0, 0, 0] }],
      ["upper_arm_r", 0.62, { rot: [22, 0, 28] }],
      ["upper_arm_r", 1.16, { rot: [18, 0, 24] }],
      ["upper_arm_r", 1.52, { rot: [0, 0, 0] }],
      ["tail_root", 0, { rot: [0, 0, 0] }],
      ["tail_root", 0.62, { rot: [0, 12, 0] }],
      ["tail_root", 1.16, { rot: [0, -12, 0] }],
      ["tail_root", 1.52, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("predator_stalk");
});
