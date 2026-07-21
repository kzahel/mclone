import { figure } from "../../src/dsl";

// A box-only adult red kangaroo with a narrow upright chest, deep haunches,
// long planted feet, short forearms, and a two-stage balancing tail. Both hind
// legs share one hop phase, unlike an alternating biped walk.
export default figure("kangaroo", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  swing,
  contactSwing,
  bob,
  followThrough,
}) => {
  mat("coat", "#a65f38");
  mat("coat_light", "#c88655");
  mat("coat_dark", "#693d2c");
  mat("cream", "#ead3ad");
  mat("muzzle", "#d7ad83");
  mat("ear_inner", "#c77f76");
  mat("eye", "#17110e");
  mat("nose", "#33221d");
  mat("claw", "#382720");

  asciiTexture("face", {
    palette: { ".": "#a65f38", "e": "#17110e", "l": "#c88655" },
    pixels: [
      "ll....ll",
      ".ee..ee.",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#d7ad83", "n": "#33221d" },
    pixels: [
      "..nnnn..",
      ".nnnnnn.",
      "...nn...",
      "........",
      "........",
    ],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#693d2c", "c": "#382720" },
    pixels: [
      "........",
      "........",
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("torso", box({
    at: [0, 1.22, 0.02],
    rot: [-4, 0, 0],
    size: [0.62, 1.05, 0.52],
    material: "coat",
  }));
  part("chest", box({
    parent: "torso",
    at: [0, 0.13, -0.3],
    size: [0.48, 0.68, 0.13],
    material: "cream",
  }));
  part("pelvis", box({
    parent: "torso",
    at: [0, -0.44, 0.16],
    size: [0.78, 0.5, 0.66],
    material: "coat_dark",
  }));
  part("belly", box({
    parent: "torso",
    at: [0, -0.31, -0.31],
    size: [0.44, 0.32, 0.12],
    material: "cream",
  }));

  part("neck", box({
    parent: "torso",
    at: [0, 0.62, -0.02],
    rot: [-7, 0, 0],
    size: [0.36, 0.42, 0.36],
    material: "coat_light",
    joint: { pivot: [0, -0.18, 0], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.33, -0.1],
    rot: [8, 0, 0],
    size: [0.46, 0.44, 0.48],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.13, -0.37],
    size: [0.32, 0.24, 0.3],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.14], ["r", 0.14]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.48, 0.06],
      rot: [-3, 0, side === "l" ? -7 : 7],
      size: [0.15, 0.64, 0.13],
      material: "coat",
      joint: { pivot: [0, -0.3, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0, -0.077],
      size: [0.075, 0.5, 0.025],
      material: "ear_inner",
    }));
  }

  for (const [side, x] of [["l", -0.38], ["r", 0.38]] as const) {
    part(`arm_${side}`, box({
      parent: "torso",
      at: [x, 0.04, -0.08],
      rot: [-18, 0, side === "l" ? -6 : 6],
      size: [0.18, 0.52, 0.2],
      material: "coat_light",
      joint: { pivot: [0, 0.26, 0], axis: [1, 0, 0] },
    }));
    part(`hand_${side}`, box({
      parent: `arm_${side}`,
      at: [0, -0.33, -0.06],
      size: [0.2, 0.17, 0.3],
      material: "coat_dark",
    }));
  }

  for (const [side, x] of [["l", -0.27], ["r", 0.27]] as const) {
    part(`leg_${side}`, box({
      parent: "torso",
      at: [x, -0.56, 0.11],
      rot: [-18, 0, 0],
      size: [0.38, 0.62, 0.48],
      material: "coat_dark",
      joint: { pivot: [0, 0.31, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.41, -0.09],
      rot: [24, 0, 0],
      size: [0.24, 0.46, 0.25],
      material: "coat_light",
    }));
    part(`foot_${side}`, box({
      parent: `shin_${side}`,
      at: [0, -0.29, -0.24],
      size: [0.32, 0.15, 0.76],
      material: "coat_dark",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  part("tail", box({
    parent: "pelvis",
    at: [0, -0.19, 0.35],
    rot: [-56, 0, 0],
    size: [0.3, 0.9, 0.3],
    material: "coat_dark",
    joint: { pivot: [0, 0.43, 0], axis: [1, 0, 0] },
  }));
  part("tail_lower", box({
    parent: "tail",
    at: [0, -0.62, 0],
    rot: [-8, 0, 0],
    size: [0.24, 0.64, 0.24],
    material: "coat_light",
  }));
  part("tail_tip", box({
    parent: "tail_lower",
    at: [0, -0.4, 0],
    size: [0.18, 0.34, 0.18],
    material: "coat_dark",
  }));

  walkCycle("hop", {
    fps: 24,
    duration: 0.82,
    samples: 25,
    loop: true,
    locomotion: {
      kind: "biped-walk",
      cycleDistance: 1.12,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("torso", { axis: "y", amount: 0.16, center: 0.16, phase: 0.5 }),
      contactSwing("leg_l", { axis: "x", degrees: 28, phase: 0.75, stanceRatio: 0.52 }),
      contactSwing("leg_r", { axis: "x", degrees: 28, phase: 0.75, stanceRatio: 0.52 }),
      followThrough("shin_l", { source: "leg_l", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 14, overshoot: 0.35, lag: 0.07 }),
      followThrough("shin_r", { source: "leg_r", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 14, overshoot: 0.35, lag: 0.07 }),
      swing("arm_l", { axis: "x", degrees: 14, center: -18, phase: 0.5 }),
      swing("arm_r", { axis: "x", degrees: 14, center: -18, phase: 0.5 }),
      followThrough("tail", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 11, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_l", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 14, overshoot: 0.65, lag: 0.14 }),
      followThrough("ear_r", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 14, overshoot: 0.65, lag: 0.14 }),
    ],
  });
});
