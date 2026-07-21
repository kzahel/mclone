import { figure } from "../../src/dsl";

// A box-only brown kiwi with a low round body, vestigial wings, sturdy legs,
// long two-stage bill, and a separate ground-directed beak-probe action.
export default figure("kiwi", ({
  asciiTexture,
  bipedWalk,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  part,
  swing,
}) => {
  mat("brown", "#69513b");
  mat("brown_light", "#8a6d4f");
  mat("brown_dark", "#44372d");
  mat("cream", "#c4a77a");
  mat("beak", "#a59678");
  mat("beak_tip", "#575149");
  mat("eye", "#171713");
  mat("leg", "#8b7b69");
  mat("foot", "#4b443d");

  asciiTexture("body_fibers", {
    palette: { ".": "#69513b", "l": "#8a6d4f", "d": "#44372d", "c": "#c4a77a" },
    pixels: [
      "llllllllll",
      "l........l",
      "..dd..dd..",
      ".d..dd..d.",
      "....cc....",
      "dd......dd",
      "dddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#8a6d4f", "d": "#44372d", "e": "#171713", "c": "#c4a77a" },
    pixels: ["dd....dd", "d.e..e.d", "..e..e..", "........", ".cccccc.", "cccccccc"],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#4b443d", "d": "#211e1b", "l": "#8b7b69" },
    pixels: ["..ll..", ".llll.", "dddddd", "dd..dd"],
  });

  part("body", box({
    at: [0, 0.72, 0.08],
    size: [0.7, 0.84, 0.84],
    material: "brown",
    faces: { east: { texture: "body_fibers" }, west: { texture: "body_fibers" } },
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.03, -0.47],
    size: [0.56, 0.62, 0.18],
    material: "brown_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.02, 0.46],
    size: [0.62, 0.66, 0.2],
    material: "brown_dark",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.39, 0.04, 0.02],
      rot: [4, 0, sign * 5],
      size: [0.1, 0.36, 0.5],
      material: "brown_dark",
      joint: { pivot: [sign * -0.035, 0.14, -0.1], axis: [0, 0, 1] },
    }));
  }
  part("neck", box({
    parent: "body",
    at: [0, 0.31, -0.35],
    rot: [-5, 0, 0],
    size: [0.36, 0.36, 0.34],
    material: "brown_dark",
    joint: { pivot: [0, -0.16, 0.12], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.23, -0.21],
    rot: [5, 0, 0],
    size: [0.54, 0.46, 0.46],
    material: "brown_light",
    faces: { north: { texture: "face" } },
  }));
  part("beak_base", box({
    parent: "head",
    at: [0, -0.08, -0.43],
    rot: [5, 0, 0],
    size: [0.2, 0.18, 0.54],
    material: "beak",
  }));
  part("beak_tip", box({
    parent: "beak_base",
    at: [0, -0.02, -0.38],
    rot: [4, 0, 0],
    size: [0.12, 0.12, 0.3],
    material: "beak_tip",
  }));
  part("tail_stub", box({
    parent: "rump",
    at: [0, 0.02, 0.16],
    rot: [10, 0, 0],
    size: [0.24, 0.16, 0.18],
    material: "brown_dark",
    joint: { pivot: [0, 0, -0.07], axis: [1, 0, 0] },
  }));

  for (const [side, x] of [["l", -0.2], ["r", 0.2]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.48, 0.01],
      size: [0.14, 0.38, 0.16],
      material: "leg",
      joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.18, -0.16],
      size: [0.28, 0.12, 0.5],
      material: "foot",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  bipedWalk("forage_walk", {
    label: "Forage walk",
    fps: 20,
    duration: 0.92,
    cycleDistance: 0.64,
    loop: true,
    samples: 21,
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.016,
    head: "head",
    headSwingDegrees: 2.3,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.65,
    swingDegrees: 17,
    tracks: [
      swing("wing_l", { axis: "z", degrees: 2.5, center: -1, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -2.5, center: 1, frequency: 2 }),
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.1 }),
      followThrough("tail_stub", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.13 }),
    ],
  });
  clip("beak_probe", {
    label: "Beak probe",
    role: "action",
    nextClip: "forage_walk",
    fps: 24,
    loop: false,
    keys: [
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.18, { rot: [-26, 0, 0] }],
      ["neck", 0.34, { rot: [-40, 0, 0] }],
      ["neck", 0.52, { rot: [-40, 0, 0] }],
      ["neck", 0.72, { rot: [-18, 0, 0] }],
      ["neck", 0.9, { rot: [0, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.18, { rot: [-12, 0, 0] }],
      ["head", 0.34, { rot: [-21, 0, 0] }],
      ["head", 0.52, { rot: [-21, 0, 0] }],
      ["head", 0.72, { rot: [-8, 0, 0] }],
      ["head", 0.9, { rot: [0, 0, 0] }],
      ["beak_base", 0, { rot: [0, 0, 0] }],
      ["beak_base", 0.34, { rot: [4, 0, 0] }],
      ["beak_base", 0.44, { rot: [-3, 0, 0] }],
      ["beak_base", 0.52, { rot: [2, 0, 0] }],
      ["beak_base", 0.9, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("forage_walk");
});
