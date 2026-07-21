import { figure } from "../../src/dsl";

// A box-only reef manta ray with a broad diamond disc, patterned dorsal
// surface, paired cephalic fins, long tail, undulating swim, and barrel roll.
export default figure("manta_ray", ({
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
  mat("back", "#334d58");
  mat("back_light", "#4d6b73");
  mat("back_dark", "#1f333b");
  mat("belly", "#d2d2c5");
  mat("mark", "#d9e0d9");
  mat("eye", "#111616");

  asciiTexture("dorsal_pattern", {
    palette: { ".": "#334d58", "l": "#4d6b73", "w": "#d9e0d9", "d": "#1f333b" },
    pixels: [
      "dddddddddddd",
      "d...wwww...d",
      "..ww....ww..",
      ".ww......ww.",
      "....llll....",
      "..llllllll..",
      "dddddddddddd",
    ],
  });
  asciiTexture("wing_pattern", {
    palette: { ".": "#334d58", "l": "#4d6b73", "w": "#d9e0d9", "d": "#1f333b" },
    pixels: [
      "dddddddddddd",
      "dww........d",
      "d.ww........",
      "d..ww..llll.",
      "d...wwlllll.",
      "dddddddddddd",
    ],
  });
  asciiTexture("belly_pattern", {
    palette: { ".": "#d2d2c5", "d": "#7d8986", "s": "#334d58" },
    pixels: ["............", "..dd....dd..", ".d........d.", "....ssss....", "............"],
  });

  part("body", box({
    at: [0, 1.0, 0.02],
    size: [0.68, 0.18, 1.12],
    material: "back",
    faces: { up: { texture: "dorsal_pattern" }, down: { texture: "belly_pattern" } },
  }));
  part("head", box({
    parent: "body",
    at: [0, 0, -0.62],
    size: [0.62, 0.2, 0.3],
    material: "back_light",
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.015, -0.24],
    size: [0.5, 0.16, 0.22],
    material: "back_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.13, -0.05],
    size: [0.54, 0.1, 0.82],
    material: "belly",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.61, 0, 0.02],
      rot: [0, sign * -10, sign * -4],
      size: [0.78, 0.12, 0.92],
      material: "back",
      faces: {
        up: { texture: "wing_pattern" },
        down: { material: "belly" },
      },
      joint: { pivot: [sign * -0.34, 0, -0.08], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [sign * 0.58, 0, 0.05],
      rot: [0, sign * -12, sign * -4],
      size: [0.5, 0.09, 0.68],
      material: "back_dark",
      faces: {
        up: { texture: "wing_pattern" },
        down: { material: "belly" },
      },
      joint: { pivot: [sign * -0.22, 0, -0.06], axis: [0, 0, 1] },
    }));
    part(`cephalic_fin_${side}`, box({
      parent: "head",
      at: [sign * 0.28, -0.04, -0.24],
      rot: [0, sign * -12, sign * 8],
      size: [0.16, 0.12, 0.44],
      material: "back_dark",
      joint: { pivot: [0, 0, 0.18], axis: [0, 1, 0] },
    }));
    part(`eye_${side}`, box({
      parent: "head",
      at: [sign * 0.32, 0.07, -0.04],
      size: [0.045, 0.08, 0.12],
      material: "eye",
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.01, 0.72],
    size: [0.18, 0.15, 0.5],
    material: "back_dark",
    joint: { pivot: [0, 0, -0.22], axis: [0, 1, 0] },
  }));
  for (const [index, width, length] of [[2, 0.13, 0.5], [3, 0.09, 0.46], [4, 0.055, 0.38]] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, 0, index === 2 ? 0.44 : index === 3 ? 0.42 : 0.35],
      size: [width, width * 0.82, length],
      material: index % 2 === 0 ? "back" : "back_dark",
      joint: { pivot: [0, 0, -length * 0.44], axis: [0, 1, 0] },
    }));
  }

  walkCycle("wing_swim", {
    label: "Wing swim",
    role: "locomotion",
    fps: 24,
    duration: 1.3,
    loop: true,
    samples: 33,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.92,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("body", { axis: "y", amount: 0.035, phase: 0.5 }),
      swing("body", { axis: "y", degrees: 2.2, phase: 0.1 }),
      swing("wing_l", { axis: "z", degrees: 22, phase: 0 }),
      swing("wing_r", { axis: "z", degrees: -22, phase: 0 }),
      swing("wing_tip_l", { axis: "z", degrees: 30, phase: 0.12 }),
      swing("wing_tip_r", { axis: "z", degrees: -30, phase: 0.12 }),
      swing("cephalic_fin_l", { axis: "y", degrees: 8, frequency: 2, phase: 0.15 }),
      swing("cephalic_fin_r", { axis: "y", degrees: -8, frequency: 2, phase: 0.15 }),
      swing("tail_1", { axis: "y", degrees: 3, phase: 0.1 }),
      swing("tail_2", { axis: "y", degrees: 5, phase: 0.2 }),
      swing("tail_3", { axis: "y", degrees: 7, phase: 0.3 }),
      swing("tail_4", { axis: "y", degrees: 9, phase: 0.4 }),
    ],
  });
  clip("barrel_roll", {
    label: "Barrel roll",
    role: "action",
    nextClip: "wing_swim",
    fps: 30,
    loop: false,
    keys: [
      ["body", 0, { rot: [0, 0, 0] }],
      ["body", 0.3, { rot: [0, 0, 90] }],
      ["body", 0.6, { rot: [0, 0, 180] }],
      ["body", 0.9, { rot: [0, 0, 270] }],
      ["body", 1.2, { rot: [0, 0, 360] }],
      ["wing_l", 0, { rot: [0, 0, 0] }],
      ["wing_l", 0.3, { rot: [0, 0, -14] }],
      ["wing_l", 0.6, { rot: [0, 0, -8] }],
      ["wing_l", 0.9, { rot: [0, 0, -14] }],
      ["wing_l", 1.2, { rot: [0, 0, 0] }],
      ["wing_r", 0, { rot: [0, 0, 0] }],
      ["wing_r", 0.3, { rot: [0, 0, 14] }],
      ["wing_r", 0.6, { rot: [0, 0, 8] }],
      ["wing_r", 0.9, { rot: [0, 0, 14] }],
      ["wing_r", 1.2, { rot: [0, 0, 0] }],
      ["tail_1", 0, { rot: [0, 0, 0] }],
      ["tail_1", 0.3, { rot: [0, 8, 0] }],
      ["tail_1", 0.6, { rot: [0, -8, 0] }],
      ["tail_1", 0.9, { rot: [0, 8, 0] }],
      ["tail_1", 1.2, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("wing_swim");
});
