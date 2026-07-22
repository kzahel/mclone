import { figure } from "../../src/dsl";

// A human-and-raptor harpy whose arms are broad feathered wings and whose
// human trunk grows directly from a compact bird pelvis above taloned legs.
export default figure("harpy", ({
  asciiTexture,
  bipedWalk,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
  swing,
}) => {
  metadata({
    bodyPlans: ["biped", "winged"],
    disposition: "neutral",
    groups: ["animal", "fantasy", "humanoid"],
    habitats: ["land", "air"],
    scale: "medium",
    themes: ["avian", "harpy", "hybrid", "humanoid", "mythic", "raptor"],
  });

  mat("skin", "#b97755");
  mat("skin_light", "#d19a75");
  mat("feather", "#74503a");
  mat("feather_light", "#aa7650");
  mat("feather_dark", "#33241e");
  mat("crest", "#c59656");
  mat("leg", "#c99a2c");
  mat("talon", "#6f501c");

  asciiTexture("harpy_face", {
    palette: { ".": "#b97755", "e": "#211512", "f": "#74503a", "l": "#d19a75" },
    pixels: [
      "ffffffffff",
      "f........f",
      "f.ee..ee.f",
      "..ee..ee..",
      "....ll....",
      "...llll...",
      "..........",
      "..........",
    ],
  });
  asciiTexture("breast_feathers", {
    palette: { ".": "#aa7650", "l": "#d0a16d", "d": "#74503a" },
    pixels: [
      "dddddddddd",
      "dlllllllld",
      "..dd..dd..",
      ".ll....ll.",
      "..llllll..",
      "dd......dd",
      "dddddddddd",
    ],
  });
  asciiTexture("wing_feathers", {
    palette: { ".": "#74503a", "l": "#aa7650", "d": "#33241e" },
    pixels: [
      "llllllllllll",
      "l..........l",
      "ll..ll..llll",
      "..ll..ll....",
      "ddd....ddddd",
      "d..dddd....d",
      "dddddddddddd",
    ],
  });
  asciiTexture("talon_front", {
    palette: { "t": "#6f501c", "c": "#18120d" },
    pixels: ["tttttttt", "tcctcctt", "cccccccc"],
  });

  part("bird_pelvis", box({
    at: [0, 0.94, 0.08],
    size: [0.68, 0.54, 0.64],
    material: "feather",
  }));
  part("tail_fan", box({
    parent: "bird_pelvis",
    at: [0, 0.02, 0.52],
    rot: [8, 0, 0],
    size: [0.78, 0.1, 0.62],
    material: "feather_dark",
    faces: { up: { texture: "wing_feathers" }, down: { texture: "wing_feathers" } },
    joint: { pivot: [0, 0, -0.28], axis: [1, 0, 0] },
  }));
  part("human_waist", box({
    parent: "bird_pelvis",
    at: [0, 0.38, -0.04],
    size: [0.42, 0.36, 0.36],
    material: "feather_light",
  }));
  part("human_torso", box({
    parent: "human_waist",
    at: [0, 0.43, -0.02],
    size: [0.6, 0.62, 0.34],
    material: "skin",
    joint: { pivot: [0, -0.29, 0], axis: [1, 0, 0] },
  }));
  part("feather_breast", box({
    parent: "human_torso",
    at: [0, -0.11, -0.2],
    size: [0.5, 0.35, 0.1],
    material: "feather_light",
    faces: { north: { texture: "breast_feathers" } },
  }));
  part("neck", box({
    parent: "human_torso",
    at: [0, 0.38, 0],
    size: [0.23, 0.22, 0.23],
    material: "skin",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.29, -0.02],
    size: [0.47, 0.47, 0.45],
    material: "skin",
    faces: { north: { texture: "harpy_face" } },
    joint: { pivot: [0, -0.2, 0], axis: [0, 1, 0] },
  }));
  part("feather_hair", box({
    parent: "head",
    at: [0, 0.18, 0.05],
    size: [0.49, 0.14, 0.48],
    material: "feather_dark",
  }));
  for (const [index, x, height] of [[1, -0.15, 0.22], [2, 0, 0.3], [3, 0.15, 0.22]] as const) {
    part(`crest_${index}`, box({
      parent: "head",
      at: [x, 0.34, 0.07],
      rot: [-5, 0, x < 0 ? -10 : x > 0 ? 10 : 0],
      size: [0.1, height, 0.13],
      material: index === 2 ? "crest" : "feather_light",
    }));
  }

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "human_torso",
      at: [sign * 0.45, 0.05, 0.06],
      rot: [-8, sign * -10, sign * 19],
      size: [0.64, 0.12, 0.92],
      material: "feather",
      faces: { up: { texture: "wing_feathers" }, down: { texture: "wing_feathers" } },
      joint: { pivot: [sign * -0.29, 0, -0.22], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [sign * 0.44, -0.01, 0.2],
      rot: [0, sign * -8, sign * 4],
      size: [0.38, 0.09, 0.78],
      material: "feather_dark",
      faces: { up: { texture: "wing_feathers" }, down: { texture: "wing_feathers" } },
    }));
    part(`leg_${side}`, box({
      parent: "bird_pelvis",
      at: [sign * 0.21, -0.44, -0.05],
      size: [0.16, 0.52, 0.18],
      material: "leg",
      joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
    }));
    part(`talon_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.33, -0.09],
      size: [0.3, 0.13, 0.4],
      material: "talon",
      faces: { north: { texture: "talon_front" } },
    }));
  }

  bipedWalk("talon_hop", {
    label: "Talon hop",
    fps: 20,
    duration: 1.06,
    cycleDistance: 0.5,
    loop: true,
    samples: 23,
    armSwingDegrees: 5,
    body: "bird_pelvis",
    bodyBob: 0.014,
    bodyBobCenter: 0.016,
    head: "head",
    headSwingDegrees: 2.5,
    leftArm: "wing_l",
    leftContact: "talon_l",
    leftLeg: "leg_l",
    rightArm: "wing_r",
    rightContact: "talon_r",
    rightLeg: "leg_r",
    stanceRatio: 0.68,
    swingDegrees: 17,
    tracks: [
      swing("human_torso", { axis: "z", degrees: 1.8, phase: 0.25 }),
      followThrough("tail_fan", { source: "bird_pelvis", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.16 }),
    ],
  });
  clip("full_threat_flare", {
    label: "Full threat flare",
    role: "action",
    nextClip: "talon_hop",
    fps: 30,
    loop: false,
    keys: [
      ["bird_pelvis", 0, { at: [0, 0, 0] }],
      ["bird_pelvis", 0.3, { at: [0, -0.04, 0] }],
      ["bird_pelvis", 0.72, { at: [0, 0.03, 0] }],
      ["bird_pelvis", 1.36, { at: [0, 0, 0] }],
      ["human_torso", 0, { rot: [0, 0, 0] }],
      ["human_torso", 0.3, { rot: [7, 0, 0] }],
      ["human_torso", 0.72, { rot: [-5, 0, 0] }],
      ["human_torso", 1.36, { rot: [0, 0, 0] }],
      ["wing_l", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_l", 0.3, { at: [-0.05, 0.03, 0], rot: [0, 0, -38] }],
      ["wing_l", 0.72, { at: [-0.1, 0.1, 0], rot: [0, 0, -68] }],
      ["wing_l", 1.36, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_r", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_r", 0.3, { at: [0.05, 0.03, 0], rot: [0, 0, 38] }],
      ["wing_r", 0.72, { at: [0.1, 0.1, 0], rot: [0, 0, 68] }],
      ["wing_r", 1.36, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["tail_fan", 0, { rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["tail_fan", 0.3, { rot: [-9, 0, 0], scale: [1.08, 1, 1.12] }],
      ["tail_fan", 0.72, { rot: [-15, 0, 0], scale: [1.18, 1, 1.18] }],
      ["tail_fan", 1.36, { rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.3, { rot: [-5, -8, 0] }],
      ["head", 0.72, { rot: [-8, 10, 0] }],
      ["head", 1.36, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("talon_hop");
});
