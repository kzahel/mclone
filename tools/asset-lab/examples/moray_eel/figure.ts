import { figure } from "../../src/dsl";

// A box-only giant moray with a mottled serpentine body, heavy jaws, exposed
// teeth, a traveling swim wave, and a separate forward jaw-lunge action.
export default figure("moray_eel", ({ asciiTexture, bob, box, clip, defaultClip, mat, part, swing, walkCycle }) => {
  mat("olive", "#6f7140");
  mat("olive_light", "#96945b");
  mat("olive_dark", "#3e472e");
  mat("belly", "#b4aa72");
  mat("mouth", "#40252a");
  mat("tooth", "#eee4c8");
  mat("eye", "#d7b74c");
  mat("pupil", "#151510");

  asciiTexture("mottle", {
    palette: { ".": "#6f7140", "l": "#96945b", "d": "#3e472e", "b": "#b4aa72" },
    pixels: ["dd..l..dd...", "d.ll..d..ll.", "..d..ll..d..", ".ll.d..ll.d.", "bbbbbbbbbbbb"],
  });
  asciiTexture("face", {
    palette: { ".": "#6f7140", "l": "#96945b", "e": "#d7b74c", "p": "#151510" },
    pixels: ["ll....ll", ".ep..pe.", "..llll..", "........", "........"],
  });

  part("head", box({
    at: [0, 0.92, -0.72],
    size: [0.64, 0.5, 0.68],
    material: "olive",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.3], axis: [0, 1, 0] },
  }));
  part("upper_jaw", box({
    parent: "head",
    at: [0, -0.02, -0.42],
    size: [0.54, 0.2, 0.3],
    material: "olive_light",
  }));
  part("lower_jaw", box({
    parent: "head",
    at: [0, -0.24, -0.38],
    rot: [3, 0, 0],
    size: [0.5, 0.15, 0.38],
    material: "mouth",
    joint: { pivot: [0, 0.05, 0.15], axis: [1, 0, 0] },
  }));
  for (const [row, parent, y, pitch] of [["u", "upper_jaw", -0.13, 0], ["l", "lower_jaw", 0.11, 180]] as const) {
    for (const [index, x] of [[1, -0.17], [2, 0], [3, 0.17]] as const) {
      part(`tooth_${row}_${index}`, box({
        parent,
        at: [x, y, -0.08],
        rot: [pitch, 0, 0],
        size: [0.055, 0.14, 0.055],
        material: "tooth",
      }));
    }
  }
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_${side}`, box({
      parent: "head",
      at: [sign * 0.325, 0.11, -0.13],
      size: [0.04, 0.11, 0.13],
      material: "eye",
      faces: { [side === "l" ? "west" : "east"]: { material: "pupil" } },
    }));
  }

  part("body_1", box({
    parent: "head",
    at: [0, -0.02, 0.54],
    size: [0.46, 0.42, 0.62],
    material: "olive_dark",
    faces: { east: { texture: "mottle" }, west: { texture: "mottle" } },
    joint: { pivot: [0, 0, -0.28], axis: [0, 1, 0] },
  }));
  for (const [index, width, height, length, material] of [
    [2, 0.44, 0.4, 0.64, "olive"],
    [3, 0.4, 0.36, 0.62, "olive_light"],
    [4, 0.35, 0.31, 0.58, "olive"],
    [5, 0.29, 0.25, 0.52, "olive_dark"],
    [6, 0.22, 0.19, 0.46, "olive"],
    [7, 0.14, 0.12, 0.38, "olive_dark"],
  ] as const) {
    part(`body_${index}`, box({
      parent: `body_${index - 1}`,
      at: [0, 0, index === 2 ? 0.54 : index === 3 ? 0.55 : index === 4 ? 0.52 : index === 5 ? 0.48 : index === 6 ? 0.42 : 0.35],
      size: [width, height, length],
      material,
      faces: { east: { texture: "mottle" }, west: { texture: "mottle" } },
      joint: { pivot: [0, 0, -length * 0.44], axis: [0, 1, 0] },
    }));
  }
  part("dorsal_fin", box({
    parent: "body_3",
    at: [0, 0.27, 0.05],
    size: [0.06, 0.22, 0.48],
    material: "olive_dark",
  }));

  walkCycle("ribbon_swim", {
    label: "Ribbon swim",
    role: "locomotion",
    fps: 24,
    duration: 1.24,
    loop: true,
    samples: 31,
    locomotion: { kind: "swim", cycleDistance: 0.86, direction: [0, 0, -1], units: "figure" },
    tracks: [
      bob("head", { axis: "y", amount: 0.025, phase: 0.5 }),
      swing("head", { axis: "y", degrees: 3, phase: 0 }),
      swing("body_1", { axis: "y", degrees: 6, phase: 0.08 }),
      swing("body_2", { axis: "y", degrees: 9, phase: 0.18 }),
      swing("body_3", { axis: "y", degrees: 12, phase: 0.28 }),
      swing("body_4", { axis: "y", degrees: 15, phase: 0.38 }),
      swing("body_5", { axis: "y", degrees: 18, phase: 0.48 }),
      swing("body_6", { axis: "y", degrees: 21, phase: 0.58 }),
      swing("body_7", { axis: "y", degrees: 24, phase: 0.68 }),
      swing("lower_jaw", { axis: "x", degrees: 2.5, center: 2.5, frequency: 2 }),
    ],
  });
  clip("jaw_lunge", {
    label: "Jaw lunge",
    role: "action",
    nextClip: "ribbon_swim",
    fps: 30,
    loop: false,
    keys: [
      ["head", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["head", 0.2, { at: [0, 0, 0.1], rot: [0, 0, 0] }],
      ["head", 0.34, { at: [0, 0, -0.24], rot: [-4, 0, 0] }],
      ["head", 0.56, { at: [0, 0, -0.12], rot: [2, 0, 0] }],
      ["head", 0.82, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["lower_jaw", 0, { rot: [0, 0, 0] }],
      ["lower_jaw", 0.2, { rot: [-10, 0, 0] }],
      ["lower_jaw", 0.34, { rot: [-32, 0, 0] }],
      ["lower_jaw", 0.48, { rot: [-4, 0, 0] }],
      ["lower_jaw", 0.82, { rot: [0, 0, 0] }],
      ["body_1", 0, { rot: [0, 0, 0] }],
      ["body_1", 0.34, { rot: [0, -10, 0] }],
      ["body_1", 0.56, { rot: [0, 6, 0] }],
      ["body_1", 0.82, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("ribbon_swim");
});
