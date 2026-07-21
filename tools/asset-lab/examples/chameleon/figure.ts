import { figure } from "../../src/dsl";

// A box-only veiled chameleon with independently raised eyes, a tall casque,
// grasping feet, an angular coiled tail, and a separate tongue-strike action.
export default figure("chameleon", ({
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
  mat("green", "#5f8b45");
  mat("green_light", "#86ad5b");
  mat("green_dark", "#365d38");
  mat("yellow", "#c6b449");
  mat("blue", "#4e8b83");
  mat("eye", "#c2b45d");
  mat("pupil", "#1a2118");
  mat("tongue", "#c76e72");

  asciiTexture("bands", {
    palette: { ".": "#5f8b45", "l": "#86ad5b", "d": "#365d38", "y": "#c6b449", "b": "#4e8b83" },
    pixels: [
      "llllllllllll",
      "l..yy...bb.l",
      "l..yy...bb.l",
      "l.d..d..d..l",
      "l...bb...y.l",
      "llllllllllll",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#86ad5b", "d": "#365d38", "y": "#c6b449", "b": "#4e8b83" },
    pixels: [
      "yy......yy",
      "y........y",
      "...bbbb...",
      "..d....d..",
      "...dddd...",
      "..........",
    ],
  });
  asciiTexture("toes", {
    palette: { ".": "#365d38", "l": "#86ad5b" },
    pixels: ["ll..ll", ".llll.", "......"],
  });

  part("body", box({
    at: [0, 0.5, 0.06],
    size: [0.58, 0.46, 1.02],
    material: "green",
    faces: {
      up: { texture: "bands" },
      east: { texture: "bands" },
      west: { texture: "bands" },
    },
  }));
  part("back_crest", box({
    parent: "body",
    at: [0, 0.31, 0.06],
    size: [0.16, 0.2, 0.78],
    material: "yellow",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.29, -0.03],
    size: [0.46, 0.12, 0.7],
    material: "green_light",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.12, -0.59],
    rot: [12, 0, 0],
    size: [0.5, 0.4, 0.34],
    material: "green_dark",
    joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.12, -0.34],
    rot: [5, 0, 0],
    size: [0.66, 0.5, 0.48],
    material: "green_light",
    faces: { north: { texture: "face" } },
  }));
  part("casque", box({
    parent: "head",
    at: [0, 0.39, 0.06],
    rot: [-12, 0, 0],
    size: [0.42, 0.38, 0.24],
    material: "yellow",
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.07, -0.31],
    size: [0.54, 0.24, 0.2],
    material: "green_light",
  }));
  part("tongue", box({
    parent: "snout",
    at: [0, -0.02, -0.12],
    size: [0.09, 0.09, 0.024],
    material: "tongue",
    joint: { pivot: [0, 0, 0.012], axis: [1, 0, 0] },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.3, 0.14, -0.08],
      size: [0.2, 0.2, 0.2],
      material: "eye",
      joint: { pivot: [sign * -0.08, 0, 0], axis: [0, 1, 0] },
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0, -0.12],
      size: [0.11, 0.11, 0.055],
      material: "pupil",
    }));
  }

  for (const [row, z, bend] of [["front", -0.31, -0.07], ["rear", 0.32, 0.07]] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "body",
        at: [sign * 0.41, -0.18, z],
        rot: [0, sign * (row === "front" ? -14 : 14), sign * 10],
        size: [0.4, 0.11, 0.14],
        material: "green_dark",
        joint: { pivot: [sign * -0.18, 0, 0], axis: [0, 1, 0] },
      }));
      part(`forearm_${row}_${side}`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.3, -0.12, bend],
        rot: [0, 0, sign * 18],
        size: [0.32, 0.09, 0.12],
        material: "green_light",
        joint: { pivot: [sign * -0.14, 0, 0], axis: [0, 1, 0] },
      }));
      part(`foot_${row}_${side}`, box({
        parent: `forearm_${row}_${side}`,
        at: [sign * 0.2, -0.06, bend],
        size: [0.24, 0.06, 0.3],
        material: "green_dark",
        faces: { up: { texture: "toes" } },
      }));
    }
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.04, 0.69],
    rot: [-10, 0, 0],
    size: [0.4, 0.3, 0.5],
    material: "green",
    joint: { pivot: [0, 0, -0.22], axis: [1, 0, 0] },
  }));
  const tailSegments = [
    ["tail_2", "tail_1", 0.41, 0.34, -18, "green_light"],
    ["tail_3", "tail_2", 0.33, 0.29, -24, "yellow"],
    ["tail_4", "tail_3", 0.27, 0.24, -30, "green_dark"],
    ["tail_5", "tail_4", 0.21, 0.19, -36, "green_light"],
    ["tail_tip", "tail_5", 0.16, 0.15, -42, "green_dark"],
  ] as const;
  for (const [name, parent, z, depth, curl, material] of tailSegments) {
    part(name, box({
      parent,
      at: [0, 0, z],
      rot: [curl, 0, 0],
      size: [Math.max(0.1, depth * 0.72), Math.max(0.1, depth * 0.56), depth],
      material,
      joint: { pivot: [0, 0, -depth * 0.44], axis: [1, 0, 0] },
    }));
  }

  walkCycle("creep", {
    label: "Careful creep",
    role: "locomotion",
    fps: 20,
    duration: 1.24,
    loop: true,
    samples: 25,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.46,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("body", { axis: "y", amount: 0.006, center: 0.006, phase: 0.5 }),
      swing("body", { axis: "y", degrees: 1.3 }),
      swing("neck", { axis: "y", degrees: 3.2, phase: 0.5 }),
      swing("eye_mound_l", { axis: "y", degrees: 11, phase: 0 }),
      swing("eye_mound_r", { axis: "y", degrees: -9, phase: 0.25 }),
      swing("leg_front_l", { axis: "y", degrees: 14, phase: 0 }),
      swing("leg_rear_r", { axis: "y", degrees: -14, phase: 0 }),
      swing("leg_front_r", { axis: "y", degrees: -14, phase: 0.5 }),
      swing("leg_rear_l", { axis: "y", degrees: 14, phase: 0.5 }),
      swing("forearm_front_l", { axis: "y", degrees: -9, phase: 0 }),
      swing("forearm_rear_r", { axis: "y", degrees: 9, phase: 0 }),
      swing("forearm_front_r", { axis: "y", degrees: 9, phase: 0.5 }),
      swing("forearm_rear_l", { axis: "y", degrees: -9, phase: 0.5 }),
      swing("tail_1", { axis: "y", degrees: 4, phase: 0.08 }),
      swing("tail_2", { axis: "y", degrees: 5, phase: 0.16 }),
    ],
  });
  clip("tongue_strike", {
    label: "Tongue strike",
    role: "action",
    nextClip: "creep",
    fps: 24,
    loop: false,
    keys: [
      ["tongue", 0, { scale: [1, 1, 1] }],
      ["tongue", 0.1, { scale: [1, 1, 1] }],
      ["tongue", 0.18, { scale: [1, 1, 26] }],
      ["tongue", 0.28, { scale: [1, 1, 26] }],
      ["tongue", 0.42, { scale: [1, 1, 1] }],
      ["tongue", 0.68, { scale: [1, 1, 1] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.18, { rot: [-5, 0, 0] }],
      ["neck", 0.42, { rot: [2, 0, 0] }],
      ["neck", 0.68, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("creep");
});
