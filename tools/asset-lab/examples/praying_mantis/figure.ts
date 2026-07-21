import { figure } from "../../src/dsl";

// A box-only European mantis with a long prothorax, triangular head, folded
// raptorial forelegs, four walking legs, veined wings, and a strike action.
export default figure("praying_mantis", ({
  asciiTexture,
  bob,
  box,
  clip,
  contactSwing,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("green", "#67894c");
  mat("green_light", "#91ad64");
  mat("green_dark", "#3f6039");
  mat("wing", "#778c4e");
  mat("brown", "#67533c");
  mat("eye", "#5d4732");
  mat("spine", "#d2c279");

  asciiTexture("abdomen_bands", {
    palette: { ".": "#67894c", "l": "#91ad64", "d": "#3f6039", "b": "#67533c" },
    pixels: [
      "llllllllll",
      "l........l",
      "dddddddddd",
      "..........",
      "bbbbbbbbbb",
      "..........",
      "dddddddddd",
    ],
  });
  asciiTexture("wing_veins", {
    palette: { ".": "#778c4e", "l": "#a8b876", "d": "#3f6039" },
    pixels: [
      "llllllllll",
      "l........l",
      ".d.d.d.d..",
      "..d.d.d.d.",
      ".d.d.d.d..",
      "dddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#91ad64", "e": "#5d4732", "h": "#d2c279", "d": "#3f6039" },
    pixels: [
      "ee......ee",
      "eh......he",
      "ee......ee",
      "...dddd...",
      "..d....d..",
      "..........",
    ],
  });

  part("abdomen", box({
    at: [0, 0.64, 0.2],
    size: [0.42, 0.34, 1.0],
    material: "green",
    faces: {
      up: { texture: "abdomen_bands" },
      east: { texture: "abdomen_bands" },
      west: { texture: "abdomen_bands" },
    },
  }));
  part("thorax", box({
    parent: "abdomen",
    at: [0, 0.08, -0.58],
    rot: [-12, 0, 0],
    size: [0.38, 0.38, 0.36],
    material: "green_dark",
  }));
  part("prothorax", box({
    parent: "thorax",
    at: [0, 0.3, -0.27],
    rot: [22, 0, 0],
    size: [0.3, 0.68, 0.26],
    material: "green_light",
    joint: { pivot: [0, -0.3, 0.1], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "prothorax",
    at: [0, 0.42, -0.08],
    rot: [-8, 0, 0],
    size: [0.56, 0.3, 0.36],
    material: "green_light",
    faces: { north: { texture: "face" } },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`antenna_${side}`, box({
      parent: "head",
      at: [sign * 0.16, 0.22, -0.05],
      rot: [-22, 0, sign * 22],
      size: [0.035, 0.38, 0.035],
      material: "brown",
      joint: { pivot: [0, -0.18, 0], axis: [0, 0, 1] },
    }));
    part(`antenna_${side}_tip`, box({
      parent: `antenna_${side}`,
      at: [0, 0.28, -0.04],
      rot: [-14, 0, sign * 7],
      size: [0.025, 0.28, 0.025],
      material: "brown",
    }));
    part(`raptor_upper_${side}`, box({
      parent: "prothorax",
      at: [sign * 0.24, 0.04, -0.17],
      rot: [-38, sign * -8, sign * 17],
      size: [0.14, 0.56, 0.14],
      material: "green_dark",
      joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
    }));
    part(`raptor_lower_${side}`, box({
      parent: `raptor_upper_${side}`,
      at: [sign * 0.02, -0.38, -0.1],
      rot: [72, 0, sign * -5],
      size: [0.11, 0.48, 0.11],
      material: "green_light",
      joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] },
    }));
    for (const [index, y] of [[1, -0.05], [2, -0.15], [3, -0.25]] as const) {
      part(`raptor_spine_${side}_${index}`, box({
        parent: `raptor_lower_${side}`,
        at: [sign * 0.08, y, -0.01],
        rot: [0, 0, sign * 25],
        size: [0.12, 0.035, 0.035],
        material: "spine",
      }));
    }
    part(`wing_${side}`, box({
      parent: "abdomen",
      at: [sign * 0.17, 0.22, 0.02],
      rot: [3, sign * -3, 0],
      size: [0.13, 0.06, 0.84],
      material: "wing",
      faces: { up: { texture: "wing_veins" } },
    }));
  }

  for (const [row, z, yaw] of [["mid", -0.1, 16], ["rear", 0.33, -20]] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "abdomen",
        at: [sign * 0.36, -0.22, z],
        rot: [0, sign * -yaw, sign * 18],
        size: [0.42, 0.065, 0.07],
        material: "brown",
        joint: { pivot: [sign * -0.19, 0, 0], axis: [0, 1, 0] },
      }));
      part(`leg_${row}_${side}_lower`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.27, -0.15, 0],
        rot: [0, 0, sign * 28],
        size: [0.34, 0.055, 0.06],
        material: "green_dark",
        joint: { pivot: [sign * -0.15, 0, 0], axis: [0, 1, 0] },
      }));
      part(`foot_${row}_${side}`, box({
        parent: `leg_${row}_${side}_lower`,
        at: [sign * 0.22, -0.04, -0.04],
        size: [0.24, 0.04, 0.16],
        material: "brown",
      }));
    }
  }

  walkCycle("stalk", {
    label: "Patient stalk",
    role: "locomotion",
    fps: 20,
    duration: 1.12,
    loop: true,
    samples: 23,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.48,
      direction: [0, 0, -1],
      units: "figure",
      contacts: [
        { part: "foot_mid_l", phaseStart: 0, phaseEnd: 0.68, role: "front-left", stanceRatio: 0.68 },
        { part: "foot_rear_r", phaseStart: 0, phaseEnd: 0.68, role: "back-right", stanceRatio: 0.68 },
        { part: "foot_mid_r", phaseStart: 0.5, phaseEnd: 0.18, role: "front-right", stanceRatio: 0.68 },
        { part: "foot_rear_l", phaseStart: 0.5, phaseEnd: 0.18, role: "back-left", stanceRatio: 0.68 },
      ],
    },
    tracks: [
      bob("abdomen", { axis: "y", amount: 0.01, center: 0.01, phase: 0.5 }),
      swing("prothorax", { axis: "y", degrees: 2.2, phase: 0.5 }),
      contactSwing("leg_mid_l", { axis: "y", degrees: 14, phase: 0, stanceRatio: 0.68 }),
      contactSwing("leg_rear_r", { axis: "y", degrees: 14, phase: 0, stanceRatio: 0.68 }),
      contactSwing("leg_mid_r", { axis: "y", degrees: 14, phase: 0.5, stanceRatio: 0.68 }),
      contactSwing("leg_rear_l", { axis: "y", degrees: 14, phase: 0.5, stanceRatio: 0.68 }),
      swing("raptor_upper_l", { axis: "x", degrees: 3, phase: 0.1 }),
      swing("raptor_upper_r", { axis: "x", degrees: 3, phase: 0.1 }),
    ],
  });
  clip("strike", {
    label: "Raptorial strike",
    role: "action",
    nextClip: "stalk",
    fps: 24,
    loop: false,
    keys: [
      ["prothorax", 0, { rot: [0, 0, 0] }],
      ["prothorax", 0.16, { rot: [7, 0, 0] }],
      ["prothorax", 0.3, { rot: [-11, 0, 0] }],
      ["prothorax", 0.52, { rot: [3, 0, 0] }],
      ["prothorax", 0.8, { rot: [0, 0, 0] }],
      ["raptor_upper_l", 0, { rot: [0, 0, 0] }],
      ["raptor_upper_l", 0.16, { rot: [18, 0, 0] }],
      ["raptor_upper_l", 0.3, { rot: [-62, 0, 0] }],
      ["raptor_upper_l", 0.52, { rot: [12, 0, 0] }],
      ["raptor_upper_l", 0.8, { rot: [0, 0, 0] }],
      ["raptor_upper_r", 0, { rot: [0, 0, 0] }],
      ["raptor_upper_r", 0.16, { rot: [18, 0, 0] }],
      ["raptor_upper_r", 0.3, { rot: [-62, 0, 0] }],
      ["raptor_upper_r", 0.52, { rot: [12, 0, 0] }],
      ["raptor_upper_r", 0.8, { rot: [0, 0, 0] }],
      ["raptor_lower_l", 0, { rot: [0, 0, 0] }],
      ["raptor_lower_l", 0.16, { rot: [-20, 0, 0] }],
      ["raptor_lower_l", 0.3, { rot: [70, 0, 0] }],
      ["raptor_lower_l", 0.52, { rot: [-12, 0, 0] }],
      ["raptor_lower_l", 0.8, { rot: [0, 0, 0] }],
      ["raptor_lower_r", 0, { rot: [0, 0, 0] }],
      ["raptor_lower_r", 0.16, { rot: [-20, 0, 0] }],
      ["raptor_lower_r", 0.3, { rot: [70, 0, 0] }],
      ["raptor_lower_r", 0.52, { rot: [-12, 0, 0] }],
      ["raptor_lower_r", 0.8, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("stalk");
});
