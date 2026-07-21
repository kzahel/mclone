import { figure } from "../../src/dsl";

// A box-only leucistic axolotl with a broad smiling head, bead eyes, six
// branched external gills, tiny paddle feet, and a tall laterally waving tail.
export default figure("axolotl", ({
  asciiTexture,
  bob,
  box,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("pink", "#e7aaa7");
  mat("pink_light", "#f3cbc0");
  mat("pink_dark", "#b86670");
  mat("gill", "#d44f70");
  mat("gill_tip", "#8f334f");
  mat("belly", "#f4d9ca");
  mat("eye", "#211b20");

  asciiTexture("body_mottle", {
    palette: { ".": "#e7aaa7", "l": "#f3cbc0", "d": "#b86670" },
    pixels: [
      "llllllllllll",
      "l..d......dl",
      "l.....d....l",
      "l.d......d.l",
      "l....d.....l",
      "llllllllllll",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#f3cbc0", "e": "#211b20", "m": "#b86670" },
    pixels: [
      "..........",
      ".ee....ee.",
      ".ee....ee.",
      "..........",
      "..m....m..",
      "...mmmm...",
    ],
  });
  asciiTexture("tail_fin", {
    palette: { ".": "#e7aaa7", "l": "#f3cbc0", "d": "#b86670" },
    pixels: [
      "dddddddddd",
      "dlllllllll",
      "dll......l",
      "d...ll...l",
      "dddddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.38, 0.02],
    size: [0.58, 0.3, 0.92],
    material: "pink",
    faces: {
      up: { texture: "body_mottle" },
      east: { texture: "body_mottle" },
      west: { texture: "body_mottle" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.18, -0.04],
    size: [0.44, 0.09, 0.66],
    material: "belly",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.01, -0.59],
    size: [0.76, 0.34, 0.5],
    material: "pink_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.21], axis: [0, 1, 0] },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.05, -0.31],
    size: [0.62, 0.16, 0.18],
    material: "pink_light",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_${side}`, box({
      parent: "head",
      at: [sign * 0.24, 0.11, -0.27],
      size: [0.1, 0.1, 0.06],
      material: "eye",
    }));
    for (const [index, y, z, angle] of [
      [1, 0.15, -0.08, 24],
      [2, 0.04, 0.01, 8],
      [3, -0.07, 0.1, -10],
    ] as const) {
      part(`gill_${side}_${index}`, box({
        parent: "head",
        at: [sign * 0.43, y, z],
        rot: [0, sign * -8, sign * angle],
        size: [0.3, 0.07, 0.09],
        material: "gill",
        joint: { pivot: [sign * -0.14, 0, 0], axis: [0, 0, 1] },
      }));
      part(`gill_tip_${side}_${index}`, box({
        parent: `gill_${side}_${index}`,
        at: [sign * 0.2, 0, 0],
        rot: [0, 0, sign * 8],
        size: [0.15, 0.1, 0.11],
        material: "gill_tip",
      }));
    }
  }

  for (const [row, z, y] of [["front", -0.28, -0.1], ["rear", 0.3, -0.08]] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "body",
        at: [sign * 0.38, y, z],
        rot: [0, sign * (row === "front" ? -12 : 12), sign * 8],
        size: [0.34, 0.08, 0.11],
        material: "pink_dark",
        joint: { pivot: [sign * -0.15, 0, 0], axis: [0, 1, 0] },
      }));
      part(`foot_${row}_${side}`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.23, -0.07, row === "front" ? -0.05 : 0.05],
        rot: [0, sign * (row === "front" ? -8 : 8), sign * 12],
        size: [0.2, 0.055, 0.22],
        material: "pink_light",
      }));
    }
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.02, 0.65],
    size: [0.44, 0.34, 0.52],
    material: "pink",
    faces: { east: { texture: "tail_fin" }, west: { texture: "tail_fin" } },
    joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, 0.03, 0.46],
    size: [0.31, 0.42, 0.46],
    material: "pink_light",
    faces: { east: { texture: "tail_fin" }, west: { texture: "tail_fin" } },
    joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] },
  }));
  part("tail_3", box({
    parent: "tail_2",
    at: [0, 0.02, 0.39],
    size: [0.2, 0.36, 0.38],
    material: "pink_dark",
    faces: { east: { texture: "tail_fin" }, west: { texture: "tail_fin" } },
    joint: { pivot: [0, 0, -0.17], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_3",
    at: [0, 0, 0.3],
    size: [0.1, 0.26, 0.27],
    material: "gill",
    joint: { pivot: [0, 0, -0.12], axis: [0, 1, 0] },
  }));

  walkCycle("swim", {
    label: "Gentle swim",
    role: "locomotion",
    fps: 24,
    duration: 1.08,
    loop: true,
    samples: 25,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.72,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("body", { axis: "y", amount: 0.018, phase: 0.5 }),
      swing("body", { axis: "y", degrees: 2.2, phase: 0 }),
      swing("head", { axis: "y", degrees: 2.8, phase: 0.5 }),
      swing("tail_1", { axis: "y", degrees: 11, phase: 0 }),
      swing("tail_2", { axis: "y", degrees: 18, phase: 0.12 }),
      swing("tail_3", { axis: "y", degrees: 24, phase: 0.24 }),
      swing("tail_tip", { axis: "y", degrees: 31, phase: 0.36 }),
      swing("leg_front_l", { axis: "y", degrees: 11, phase: 0 }),
      swing("leg_rear_r", { axis: "y", degrees: -11, phase: 0 }),
      swing("leg_front_r", { axis: "y", degrees: -11, phase: 0.5 }),
      swing("leg_rear_l", { axis: "y", degrees: 11, phase: 0.5 }),
      swing("gill_l_1", { axis: "z", degrees: 5, phase: 0.06 }),
      swing("gill_l_2", { axis: "z", degrees: 6, phase: 0.12 }),
      swing("gill_l_3", { axis: "z", degrees: 7, phase: 0.18 }),
      swing("gill_r_1", { axis: "z", degrees: -5, phase: 0.06 }),
      swing("gill_r_2", { axis: "z", degrees: -6, phase: 0.12 }),
      swing("gill_r_3", { axis: "z", degrees: -7, phase: 0.18 }),
    ],
  });
  defaultClip("swim");
});
