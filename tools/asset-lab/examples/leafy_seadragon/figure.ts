import { figure } from "../../src/dsl";

// A box-only leafy seadragon with an upright horse-like head, narrow plated
// trunk, long tail, and drifting leaf appendages with a separate fan action.
export default figure("leafy_seadragon", ({
  asciiTexture,
  bob,
  box,
  clip,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("body", "#b08b45");
  mat("body_light", "#d1b465");
  mat("body_dark", "#6e5a34");
  mat("leaf", "#9d8d43");
  mat("leaf_light", "#c4b65a");
  mat("leaf_dark", "#5f6636");
  mat("eye", "#d8c76c");
  mat("pupil", "#171812");

  asciiTexture("plates", {
    palette: { ".": "#b08b45", "l": "#d1b465", "d": "#6e5a34" },
    pixels: [
      "dddddddddddd",
      "d.ll..ll...d",
      "d...ll..ll.d",
      "dll...ll...d",
      "d..ll...ll.d",
      "dddddddddddd",
    ],
  });
  asciiTexture("leaf_pattern", {
    palette: { ".": "#9d8d43", "l": "#c4b65a", "d": "#5f6636" },
    pixels: [
      "....ll....",
      "...llll...",
      "..llddll..",
      ".llddddll.",
      "llddddddl.",
      ".llddddll.",
      "..llddll..",
      "...llll...",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#d1b465", "d": "#6e5a34", "m": "#333123" },
    pixels: ["........", "..dddd..", "........", "...mm..."],
  });

  part("trunk", box({
    at: [0, 1.12, 0.02],
    size: [0.38, 0.44, 1.1],
    material: "body",
    faces: { east: { texture: "plates" }, west: { texture: "plates" } },
  }));
  part("belly", box({
    parent: "trunk",
    at: [0, -0.22, -0.08],
    size: [0.28, 0.12, 0.72],
    material: "body_light",
  }));
  part("neck", box({
    parent: "trunk",
    at: [0, 0.3, -0.48],
    rot: [-24, 0, 0],
    size: [0.26, 0.5, 0.3],
    material: "body_light",
    joint: { pivot: [0, -0.2, 0.1], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.32, -0.18],
    rot: [12, 0, 0],
    size: [0.42, 0.34, 0.4],
    material: "body",
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.05, -0.38],
    size: [0.2, 0.16, 0.48],
    material: "body_light",
    faces: { north: { texture: "snout_face" } },
  }));
  part("snout_tip", box({
    parent: "snout",
    at: [0, 0, -0.28],
    size: [0.17, 0.13, 0.12],
    material: "body_dark",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.18, 0.07, -0.12],
      size: [0.15, 0.15, 0.17],
      material: "body_dark",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0.01, -0.1],
      size: [0.09, 0.09, 0.04],
      material: "eye",
      faces: { north: { material: "pupil" } },
    }));
  }

  for (const [index, x, z, height, depth, pitch] of [
    [1, -0.018, -0.35, 0.38, 0.28, -18],
    [2, 0, -0.02, 0.46, 0.32, 12],
    [3, 0.018, 0.34, 0.36, 0.3, -10],
  ] as const) {
    part(`dorsal_leaf_${index}`, box({
      parent: "trunk",
      at: [x, 0.31, z],
      rot: [pitch, 0, 0],
      size: [0.06, height, depth],
      material: index === 2 ? "leaf_light" : "leaf",
      faces: { east: { texture: "leaf_pattern" }, west: { texture: "leaf_pattern" } },
      joint: { pivot: [0, -height * 0.42, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    for (const [index, z, width, depth, tilt] of [
      [1, -0.28, 0.34, 0.27, 16],
      [2, 0.22, 0.42, 0.3, -12],
    ] as const) {
      part(`side_leaf_${side}_${index}`, box({
        parent: "trunk",
        at: [sign * 0.28, -0.02, z],
        rot: [0, sign * tilt, sign * 10],
        size: [width, 0.055, depth],
        material: index === 1 ? "leaf_dark" : "leaf",
        faces: { up: { texture: "leaf_pattern" }, down: { texture: "leaf_pattern" } },
        joint: { pivot: [sign * -width * 0.42, 0, 0], axis: [0, 0, 1] },
      }));
    }
  }

  part("head_leaf", box({
    parent: "head",
    at: [0, 0.25, 0.04],
    rot: [-20, 0, 0],
    size: [0.055, 0.32, 0.23],
    material: "leaf_light",
    faces: { east: { texture: "leaf_pattern" }, west: { texture: "leaf_pattern" } },
    joint: { pivot: [0, -0.13, 0], axis: [1, 0, 0] },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`jaw_leaf_${side}`, box({
      parent: "head",
      at: [sign * 0.22, -0.12, 0.02],
      rot: [0, sign * 12, sign * 22],
      size: [0.28, 0.05, 0.2],
      material: "leaf_dark",
      faces: { up: { texture: "leaf_pattern" }, down: { texture: "leaf_pattern" } },
      joint: { pivot: [sign * -0.12, 0, 0], axis: [0, 0, 1] },
    }));
  }

  part("tail_1", box({
    parent: "trunk",
    at: [0, -0.02, 0.68],
    rot: [0, 8, 0],
    size: [0.32, 0.34, 0.42],
    material: "body_dark",
    joint: { pivot: [0, 0, -0.18], axis: [0, 1, 0] },
  }));
  for (const [index, width, length, yaw, material] of [
    [2, 0.27, 0.38, 12, "body"],
    [3, 0.22, 0.34, 16, "body_light"],
    [4, 0.17, 0.3, 20, "body_dark"],
    [5, 0.12, 0.25, 24, "body_light"],
  ] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, 0, index === 2 ? 0.35 : index === 3 ? 0.31 : index === 4 ? 0.28 : 0.24],
      rot: [0, yaw, 0],
      size: [width, width, length],
      material,
      joint: { pivot: [0, 0, -length * 0.42], axis: [0, 1, 0] },
    }));
  }
  part("tail_leaf", box({
    parent: "tail_3",
    at: [0, 0.22, 0.06],
    rot: [-8, 0, 0],
    size: [0.05, 0.34, 0.24],
    material: "leaf",
    faces: { east: { texture: "leaf_pattern" }, west: { texture: "leaf_pattern" } },
    joint: { pivot: [0, -0.14, 0], axis: [1, 0, 0] },
  }));

  walkCycle("hover_swim", {
    label: "Leafy hover",
    role: "locomotion",
    fps: 24,
    duration: 1.42,
    loop: true,
    samples: 35,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.3,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("trunk", { axis: "y", amount: 0.04, phase: 0.5 }),
      swing("trunk", { axis: "y", degrees: 2.4, phase: 0.15 }),
      swing("neck", { axis: "y", degrees: 4, phase: 0.62 }),
      swing("dorsal_leaf_1", { axis: "x", degrees: 12, frequency: 1.5, phase: 0.05 }),
      swing("dorsal_leaf_2", { axis: "x", degrees: 14, frequency: 1.5, phase: 0.2 }),
      swing("dorsal_leaf_3", { axis: "x", degrees: 12, frequency: 1.5, phase: 0.35 }),
      swing("side_leaf_l_1", { axis: "z", degrees: 12, frequency: 1.5, phase: 0 }),
      swing("side_leaf_r_1", { axis: "z", degrees: -12, frequency: 1.5, phase: 0 }),
      swing("side_leaf_l_2", { axis: "z", degrees: 15, frequency: 1.5, phase: 0.25 }),
      swing("side_leaf_r_2", { axis: "z", degrees: -15, frequency: 1.5, phase: 0.25 }),
      swing("head_leaf", { axis: "x", degrees: 10, frequency: 1.5, phase: 0.4 }),
      swing("jaw_leaf_l", { axis: "z", degrees: 9, frequency: 1.5, phase: 0.5 }),
      swing("jaw_leaf_r", { axis: "z", degrees: -9, frequency: 1.5, phase: 0.5 }),
      swing("tail_1", { axis: "y", degrees: 4, phase: 0 }),
      swing("tail_2", { axis: "y", degrees: 7, phase: 0.12 }),
      swing("tail_3", { axis: "y", degrees: 10, phase: 0.24 }),
      swing("tail_4", { axis: "y", degrees: 13, phase: 0.36 }),
      swing("tail_5", { axis: "y", degrees: 16, phase: 0.48 }),
      swing("tail_leaf", { axis: "x", degrees: 12, frequency: 1.5, phase: 0.3 }),
    ],
  });
  clip("leaf_fan", {
    label: "Leaf fan",
    role: "action",
    nextClip: "hover_swim",
    fps: 30,
    loop: false,
    keys: [
      ["dorsal_leaf_1", 0, { rot: [0, 0, 0] }],
      ["dorsal_leaf_1", 0.42, { rot: [-24, 0, 0] }],
      ["dorsal_leaf_1", 0.72, { rot: [-30, 0, 0] }],
      ["dorsal_leaf_1", 1.08, { rot: [0, 0, 0] }],
      ["dorsal_leaf_2", 0, { rot: [0, 0, 0] }],
      ["dorsal_leaf_2", 0.42, { rot: [22, 0, 0] }],
      ["dorsal_leaf_2", 0.72, { rot: [28, 0, 0] }],
      ["dorsal_leaf_2", 1.08, { rot: [0, 0, 0] }],
      ["dorsal_leaf_3", 0, { rot: [0, 0, 0] }],
      ["dorsal_leaf_3", 0.42, { rot: [-20, 0, 0] }],
      ["dorsal_leaf_3", 0.72, { rot: [-26, 0, 0] }],
      ["dorsal_leaf_3", 1.08, { rot: [0, 0, 0] }],
      ["side_leaf_l_1", 0, { rot: [0, 0, 0] }],
      ["side_leaf_l_1", 0.5, { rot: [0, 0, -24] }],
      ["side_leaf_l_1", 0.76, { rot: [0, 0, -30] }],
      ["side_leaf_l_1", 1.08, { rot: [0, 0, 0] }],
      ["side_leaf_r_1", 0, { rot: [0, 0, 0] }],
      ["side_leaf_r_1", 0.5, { rot: [0, 0, 24] }],
      ["side_leaf_r_1", 0.76, { rot: [0, 0, 30] }],
      ["side_leaf_r_1", 1.08, { rot: [0, 0, 0] }],
      ["side_leaf_l_2", 0, { rot: [0, 0, 0] }],
      ["side_leaf_l_2", 0.5, { rot: [0, 0, -28] }],
      ["side_leaf_l_2", 0.76, { rot: [0, 0, -34] }],
      ["side_leaf_l_2", 1.08, { rot: [0, 0, 0] }],
      ["side_leaf_r_2", 0, { rot: [0, 0, 0] }],
      ["side_leaf_r_2", 0.5, { rot: [0, 0, 28] }],
      ["side_leaf_r_2", 0.76, { rot: [0, 0, 34] }],
      ["side_leaf_r_2", 1.08, { rot: [0, 0, 0] }],
      ["head_leaf", 0, { rot: [0, 0, 0] }],
      ["head_leaf", 0.6, { rot: [-22, 0, 0] }],
      ["head_leaf", 1.08, { rot: [0, 0, 0] }],
      ["tail_leaf", 0, { rot: [0, 0, 0] }],
      ["tail_leaf", 0.6, { rot: [24, 0, 0] }],
      ["tail_leaf", 1.08, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("hover_swim");
});
