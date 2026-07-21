import { figure } from "../../src/dsl";

// A box-only great hornbill with black-white plumage, a huge yellow bill and
// casque, broad wings, long tail, and a separate lower-bill clack action.
export default figure("great_hornbill", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, wingFlap }) => {
  mat("black", "#202322");
  mat("black_light", "#3b403d");
  mat("white", "#eee9d8");
  mat("yellow", "#dfae3e");
  mat("yellow_light", "#f1ca62");
  mat("orange", "#d97932");
  mat("red", "#a74432");
  mat("eye", "#171817");
  mat("foot", "#77756a");

  asciiTexture("face", {
    palette: { ".": "#202322", "w": "#eee9d8", "e": "#171817", "r": "#a74432" },
    pixels: ["........", ".we..ew.", ".we..ew.", "..rrrr..", "wwwwwwww"],
  });
  asciiTexture("bill_side", {
    palette: { "y": "#dfae3e", "l": "#f1ca62", "o": "#d97932", "d": "#8a5b2b" },
    pixels: ["llllllllllll", "lyyyyyyyyyyd", "lyyoooooyyyd", "yyyyoooooyyd", "oooooooooodd"],
  });
  asciiTexture("wing", {
    palette: { ".": "#202322", "l": "#3b403d", "w": "#eee9d8" },
    pixels: ["llllllllll", "l........l", "..wwwwww..", ".wwwwwwww.", "..........", "llllllllll"],
  });
  asciiTexture("tail", {
    palette: { ".": "#eee9d8", "b": "#202322" },
    pixels: ["..........", "..........", "bbbbbbbbbb", "..........", ".........."],
  });

  part("body", box({ at: [0, 0.92, 0.06], size: [0.72, 0.72, 0.92], material: "black" }));
  part("breast", box({ parent: "body", at: [0, -0.03, -0.5], size: [0.58, 0.56, 0.18], material: "white" }));
  part("throat", box({ parent: "body", at: [0, 0.34, -0.34], size: [0.5, 0.34, 0.34], material: "white" }));
  part("head", box({
    parent: "throat",
    at: [0, 0.24, -0.15],
    size: [0.58, 0.46, 0.46],
    material: "black_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.19, 0.14], axis: [1, 0, 0] },
  }));
  part("upper_bill", box({
    parent: "head",
    at: [0, 0.01, -0.54],
    size: [0.5, 0.34, 0.82],
    material: "yellow",
    faces: { east: { texture: "bill_side" }, west: { texture: "bill_side" } },
  }));
  part("bill_tip", box({ parent: "upper_bill", at: [0, -0.03, -0.46], rot: [6, 0, 0], size: [0.34, 0.24, 0.16], material: "orange" }));
  part("lower_bill", box({
    parent: "head",
    at: [0, -0.2, -0.49],
    size: [0.42, 0.12, 0.7],
    material: "orange",
    joint: { pivot: [0, 0.04, 0.28], axis: [1, 0, 0] },
  }));
  part("casque_base", box({ parent: "head", at: [0, 0.31, -0.2], rot: [-8, 0, 0], size: [0.4, 0.22, 0.52], material: "yellow_light" }));
  part("casque_crown", box({ parent: "casque_base", at: [0, 0.17, 0.06], rot: [-10, 0, 0], size: [0.3, 0.18, 0.36], material: "orange" }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.55, 0.08, 0.04],
      size: [0.92, 0.08, 0.82],
      material: "black_light",
      faces: { up: { texture: "wing" }, down: { texture: "wing" } },
      joint: { pivot: [sign * -0.44, 0, -0.14], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [sign * 0.66, 0.015, 0.06],
      rot: [0, sign * -7, 0],
      size: [0.5, 0.06, 0.62],
      material: "black",
      faces: { up: { texture: "wing" }, down: { texture: "wing" } },
    }));
    part(`leg_${side}`, box({ parent: "body", at: [sign * 0.18, -0.44, -0.01], rot: [-30, 0, 0], size: [0.11, 0.26, 0.12], material: "foot" }));
    part(`foot_${side}`, box({ parent: `leg_${side}`, at: [0, -0.14, 0.08], size: [0.22, 0.08, 0.28], material: "foot" }));
  }
  part("tail_base", box({
    parent: "body",
    at: [0, 0.02, 0.62],
    size: [0.58, 0.09, 0.5],
    material: "white",
    faces: { up: { texture: "tail" }, down: { texture: "tail" } },
    joint: { pivot: [0, 0, -0.22], axis: [1, 0, 0] },
  }));
  part("tail_tip", box({ parent: "tail_base", at: [0, 0, 0.41], size: [0.5, 0.08, 0.42], material: "white", faces: { up: { texture: "tail" }, down: { texture: "tail" } } }));

  wingFlap("forest_flight", {
    label: "Forest flight",
    fps: 20,
    duration: 0.98,
    cycleDistance: 1.26,
    loop: true,
    samples: 21,
    body: "body",
    bodyBob: 0.04,
    degrees: 34,
    leftWing: "wing_l",
    rightWing: "wing_r",
    tracks: [
      followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.11 }),
      followThrough("tail_base", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.15 }),
    ],
  });
  clip("beak_clack", {
    label: "Beak clack",
    role: "action",
    nextClip: "forest_flight",
    fps: 30,
    loop: false,
    keys: [
      ["lower_bill", 0, { rot: [0, 0, 0] }],
      ["lower_bill", 0.16, { rot: [-22, 0, 0] }],
      ["lower_bill", 0.28, { rot: [0, 0, 0] }],
      ["lower_bill", 0.42, { rot: [-18, 0, 0] }],
      ["lower_bill", 0.52, { rot: [0, 0, 0] }],
      ["lower_bill", 0.72, { rot: [0, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.16, { rot: [-5, 0, 0] }],
      ["head", 0.28, { rot: [3, 0, 0] }],
      ["head", 0.42, { rot: [-4, 0, 0] }],
      ["head", 0.72, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("forest_flight");
});
