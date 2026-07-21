import { figure } from "../../src/dsl";

// A box-only southern cassowary with a deep black body, cobalt neck, red
// wattles, tall casque, powerful legs, and a separate defensive kick action.
export default figure("cassowary", ({
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
  mat("black", "#202423");
  mat("black_light", "#363c39");
  mat("black_blue", "#29363a");
  mat("blue", "#356e86");
  mat("blue_light", "#5795a7");
  mat("red", "#b9473d");
  mat("red_dark", "#7a2f2d");
  mat("casque", "#a68c55");
  mat("casque_dark", "#675a3e");
  mat("beak", "#5a5a4d");
  mat("leg", "#9a8d77");
  mat("foot", "#4c483f");

  asciiTexture("body_plumes", {
    palette: { ".": "#202423", "l": "#363c39", "b": "#29363a" },
    pixels: [
      "llllllllllll",
      "l..........l",
      "..bb..bb....",
      ".b..bb..b...",
      "............",
      "bbbbbbbbbbbb",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#356e86", "l": "#5795a7", "e": "#171916", "d": "#29363a" },
    pixels: ["dddddddd", "d.e..e.d", "d.e..e.d", ".l....l.", "..llll..", "........"],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#4c483f", "d": "#26231f", "c": "#8a806e" },
    pixels: ["...cc...", "..cccc..", "dddddddd", "dd....dd"],
  });

  part("body", box({
    at: [0, 1.57, 0.08],
    size: [0.92, 0.9, 1.08],
    material: "black",
    faces: { east: { texture: "body_plumes" }, west: { texture: "body_plumes" } },
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.05, -0.59],
    size: [0.7, 0.7, 0.2],
    material: "black_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.02, 0.55],
    size: [0.82, 0.74, 0.22],
    material: "black_blue",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.5, 0.02, -0.02],
      size: [0.12, 0.5, 0.72],
      material: "black_light",
      faces: { [side === "l" ? "west" : "east"]: { texture: "body_plumes" } },
      joint: { pivot: [sign * -0.04, 0.18, -0.2], axis: [0, 0, 1] },
    }));
    part(`wing_quill_${side}`, box({
      parent: `wing_${side}`,
      at: [0, -0.27, 0.15],
      size: [0.08, 0.32, 0.48],
      material: "black_blue",
    }));
  }
  part("neck_lower", box({
    parent: "body",
    at: [0, 0.61, -0.4],
    rot: [-10, 0, 0],
    size: [0.38, 0.68, 0.38],
    material: "blue",
    joint: { pivot: [0, -0.32, 0.08], axis: [1, 0, 0] },
  }));
  part("neck_upper", box({
    parent: "neck_lower",
    at: [0, 0.45, -0.08],
    rot: [8, 0, 0],
    size: [0.34, 0.46, 0.34],
    material: "blue_light",
  }));
  part("head", box({
    parent: "neck_upper",
    at: [0, 0.34, -0.13],
    rot: [5, 0, 0],
    size: [0.52, 0.44, 0.5],
    material: "blue",
    faces: { north: { texture: "face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.06, -0.37],
    size: [0.38, 0.2, 0.28],
    material: "beak",
  }));
  part("beak_tip", box({
    parent: "beak",
    at: [0, -0.02, -0.18],
    size: [0.28, 0.15, 0.12],
    material: "casque_dark",
  }));
  part("casque_base", box({
    parent: "head",
    at: [0, 0.29, 0.04],
    rot: [-6, 0, 0],
    size: [0.36, 0.24, 0.34],
    material: "casque",
  }));
  part("casque_crown", box({
    parent: "casque_base",
    at: [0, 0.2, 0.04],
    rot: [-8, 0, 0],
    size: [0.26, 0.24, 0.26],
    material: "casque_dark",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wattle_${side}`, box({
      parent: "head",
      at: [sign * 0.1, -0.29, -0.05],
      rot: [8, 0, sign * 5],
      size: [0.11, side === "l" ? 0.38 : 0.31, 0.12],
      material: side === "l" ? "red" : "red_dark",
      joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
    }));
  }
  part("tail", box({
    parent: "rump",
    at: [0, 0.08, 0.21],
    rot: [8, 0, 0],
    size: [0.56, 0.16, 0.42],
    material: "black_blue",
    joint: { pivot: [0, 0, -0.18], axis: [1, 0, 0] },
  }));

  for (const [side, x] of [["l", -0.29], ["r", 0.29]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.72, 0.02],
      size: [0.21, 0.72, 0.22],
      material: "leg",
      joint: { pivot: [0, 0.35, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.5, 0.05],
      rot: [3, 0, 0],
      size: [0.16, 0.6, 0.18],
      material: "leg",
    }));
    part(`foot_${side}`, box({
      parent: `shin_${side}`,
      at: [0, -0.28, -0.23],
      size: [0.34, 0.14, 0.72],
      material: "foot",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  bipedWalk("forest_run", {
    label: "Forest run",
    fps: 22,
    duration: 0.78,
    cycleDistance: 1.18,
    loop: true,
    samples: 21,
    body: "body",
    bodyBob: 0.023,
    bodyBobCenter: 0.025,
    head: "head",
    headSwingDegrees: 2.6,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.58,
    swingDegrees: 23,
    tracks: [
      swing("wing_l", { axis: "z", degrees: 4, center: -2, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -4, center: 2, frequency: 2 }),
      followThrough("neck_lower", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4.5, overshoot: 0.5, lag: 0.1 }),
      followThrough("wattle_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.12 }),
      followThrough("wattle_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.6, lag: 0.14 }),
    ],
  });
  clip("kick", {
    label: "Defensive kick",
    role: "action",
    nextClip: "forest_run",
    fps: 30,
    loop: false,
    keys: [
      ["neck_lower", 0, { rot: [0, 0, 0] }],
      ["neck_lower", 0.16, { rot: [-8, 0, 0] }],
      ["neck_lower", 0.28, { rot: [5, 0, 0] }],
      ["neck_lower", 0.62, { rot: [0, 0, 0] }],
      ["leg_l", 0, { rot: [0, 0, 0] }],
      ["leg_l", 0.16, { rot: [-12, 0, 0] }],
      ["leg_l", 0.28, { rot: [58, 0, 0] }],
      ["leg_l", 0.38, { rot: [66, 0, 0] }],
      ["leg_l", 0.5, { rot: [18, 0, 0] }],
      ["leg_l", 0.62, { rot: [0, 0, 0] }],
      ["shin_l", 0, { rot: [0, 0, 0] }],
      ["shin_l", 0.16, { rot: [18, 0, 0] }],
      ["shin_l", 0.28, { rot: [-22, 0, 0] }],
      ["shin_l", 0.38, { rot: [-10, 0, 0] }],
      ["shin_l", 0.5, { rot: [8, 0, 0] }],
      ["shin_l", 0.62, { rot: [0, 0, 0] }],
      ["wing_l", 0, { rot: [0, 0, 0] }],
      ["wing_l", 0.2, { rot: [0, 0, -16] }],
      ["wing_l", 0.4, { rot: [0, 0, -8] }],
      ["wing_l", 0.62, { rot: [0, 0, 0] }],
      ["wing_r", 0, { rot: [0, 0, 0] }],
      ["wing_r", 0.2, { rot: [0, 0, 16] }],
      ["wing_r", 0.4, { rot: [0, 0, 8] }],
      ["wing_r", 0.62, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("forest_run");
});
