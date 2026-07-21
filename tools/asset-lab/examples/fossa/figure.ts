import { figure } from "../../src/dsl";

// A box-only adult fossa with a long tawny body, low feline head, rounded
// ears, oversized balancing tail, quiet prowl, and a separate pounce action.
export default figure("fossa", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk }) => {
  mat("coat", "#a96539");
  mat("coat_light", "#c07b49");
  mat("coat_dark", "#69402e");
  mat("cream", "#dfc49c");
  mat("nose", "#27211e");
  mat("eye", "#d1a13e");
  mat("paw", "#4c342a");

  asciiTexture("coat_side", {
    palette: { ".": "#a96539", "l": "#c07b49", "d": "#69402e", "c": "#dfc49c" },
    pixels: ["dddddddddddd", "d..........d", "..llllllll..", ".l........l.", "..cccccccc.."],
  });
  asciiTexture("face", {
    palette: { ".": "#a96539", "l": "#c07b49", "e": "#d1a13e", "d": "#69402e" },
    pixels: ["dd....dd", ".le..el.", "..llll..", ".llllll.", "........"],
  });
  asciiTexture("muzzle", {
    palette: { ".": "#dfc49c", "n": "#27211e" },
    pixels: ["........", "..nnnn..", ".nnnnnn.", "...nn..."] ,
  });

  part("body", box({
    at: [0, 0.76, 0.04],
    size: [0.68, 0.5, 1.34],
    material: "coat",
    faces: { east: { texture: "coat_side" }, west: { texture: "coat_side" } },
  }));
  part("belly", box({ parent: "body", at: [0, -0.29, -0.02], size: [0.5, 0.11, 0.98], material: "cream" }));
  part("shoulders", box({ parent: "body", at: [0, 0.05, -0.55], size: [0.72, 0.48, 0.38], material: "coat_light" }));
  part("neck", box({
    parent: "body",
    at: [0, 0.05, -0.72],
    size: [0.46, 0.4, 0.34],
    material: "coat_dark",
    joint: { pivot: [0, 0, 0.14], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.04, -0.32],
    size: [0.56, 0.44, 0.46],
    material: "coat_light",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({ parent: "head", at: [0, -0.1, -0.32], size: [0.34, 0.2, 0.2], material: "cream", faces: { north: { texture: "muzzle" } } }));
  part("nose", box({ parent: "muzzle", at: [0, -0.01, -0.13], size: [0.2, 0.13, 0.08], material: "nose" }));
  for (const [side, x] of [["l", -0.2], ["r", 0.2]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.27, 0.05],
      rot: [0, 0, side === "l" ? -7 : 7],
      size: [0.16, 0.18, 0.12],
      material: "coat_dark",
      joint: { pivot: [0, -0.07, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [["fl", -0.22, -0.42], ["fr", 0.22, -0.42], ["bl", -0.23, 0.44], ["br", 0.23, 0.44]] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.4, z],
      size: [0.15, 0.38, 0.17],
      material: suffix.startsWith("b") ? "coat_dark" : "coat",
      joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({ parent: `leg_${suffix}`, at: [0, -0.24, -0.05], size: [0.22, 0.11, 0.28], material: "paw" }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.05, 0.82],
    size: [0.28, 0.25, 0.56],
    material: "coat_dark",
    joint: { pivot: [0, 0, -0.25], axis: [0, 1, 0] },
  }));
  for (const [index, width, length, material] of [[2, 0.24, 0.54, "coat"], [3, 0.19, 0.5, "coat_light"], [4, 0.13, 0.42, "coat_dark"]] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, 0, index === 2 ? 0.48 : index === 3 ? 0.46 : 0.39],
      size: [width, width * 0.88, length],
      material,
      joint: { pivot: [0, 0, -length * 0.44], axis: [0, 1, 0] },
    }));
  }

  quadrupedWalk("forest_prowl", {
    label: "Forest prowl",
    fps: 20,
    duration: 1.06,
    cycleDistance: 0.82,
    gait: "walk",
    loop: true,
    samples: 21,
    contactParts: { frontLeft: "paw_fl", frontRight: "paw_fr", backLeft: "paw_bl", backRight: "paw_br" },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 2.4,
    legs: { frontLeft: "leg_fl", frontRight: "leg_fr", backLeft: "leg_bl", backRight: "leg_br" },
    stanceRatio: 0.68,
    swingDegrees: 18,
    tail: "tail_1",
    tailSwingDegrees: 9,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.5, lag: 0.12 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 11, overshoot: 0.72, lag: 0.14 }),
      followThrough("tail_3", { source: "tail_2", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 13, overshoot: 0.76, lag: 0.17 }),
    ],
  });
  clip("pounce", {
    label: "Pounce",
    role: "action",
    nextClip: "forest_prowl",
    fps: 30,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0] }],
      ["body", 0.18, { at: [0, -0.03, 0.08] }],
      ["body", 0.38, { at: [0, 0.32, -0.18] }],
      ["body", 0.58, { at: [0, 0.42, -0.42] }],
      ["body", 0.82, { at: [0, 0.12, -0.58] }],
      ["body", 1.02, { at: [0, 0, 0] }],
      ["leg_fl", 0, { rot: [0, 0, 0] }], ["leg_fl", 0.38, { rot: [42, 0, 0] }], ["leg_fl", 0.68, { rot: [-12, 0, 0] }], ["leg_fl", 1.02, { rot: [0, 0, 0] }],
      ["leg_fr", 0, { rot: [0, 0, 0] }], ["leg_fr", 0.38, { rot: [42, 0, 0] }], ["leg_fr", 0.68, { rot: [-12, 0, 0] }], ["leg_fr", 1.02, { rot: [0, 0, 0] }],
      ["leg_bl", 0, { rot: [0, 0, 0] }], ["leg_bl", 0.38, { rot: [-32, 0, 0] }], ["leg_bl", 0.68, { rot: [24, 0, 0] }], ["leg_bl", 1.02, { rot: [0, 0, 0] }],
      ["leg_br", 0, { rot: [0, 0, 0] }], ["leg_br", 0.38, { rot: [-32, 0, 0] }], ["leg_br", 0.68, { rot: [24, 0, 0] }], ["leg_br", 1.02, { rot: [0, 0, 0] }],
      ["tail_1", 0, { rot: [0, 0, 0] }], ["tail_1", 0.58, { rot: [-12, 0, 0] }], ["tail_1", 1.02, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("forest_prowl");
});
