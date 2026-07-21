import { figure } from "../../src/dsl";

// A box-only adult secretary bird with a pale gray body, black flight
// feathers, orange facial skin, swept crest, long legs, and a separate stomp.
export default figure("secretary_bird", ({
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
  mat("gray", "#9da5a1");
  mat("gray_light", "#c2c8c2");
  mat("gray_dark", "#626a68");
  mat("black", "#272b2b");
  mat("black_light", "#3d4341");
  mat("face_orange", "#d77a36");
  mat("face_red", "#a94c35");
  mat("beak", "#5c615d");
  mat("beak_tip", "#303431");
  mat("leg", "#d39a6b");
  mat("foot", "#615448");

  asciiTexture("wing_bars", {
    palette: { ".": "#626a68", "l": "#9da5a1", "b": "#272b2b" },
    pixels: [
      "llllllllll",
      "l........l",
      "..bbbbbb..",
      "..........",
      "bbbbbbbbbb",
      "bbbbbbbbbb",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#c2c8c2", "o": "#d77a36", "r": "#a94c35", "e": "#171918" },
    pixels: [
      "........",
      ".oo..oo.",
      ".oe..eo.",
      ".rr..rr.",
      "..oooo..",
      "........",
    ],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#615448", "d": "#332c27", "l": "#8a7765" },
    pixels: ["...ll...", "..llll..", "dddddddd", "dd....dd"],
  });

  part("body", box({
    at: [0, 1.52, 0.06],
    size: [0.74, 0.72, 1.0],
    material: "gray",
  }));
  part("breast", box({
    parent: "body",
    at: [0, 0.02, -0.55],
    size: [0.56, 0.58, 0.18],
    material: "gray_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.02, 0.52],
    size: [0.66, 0.58, 0.2],
    material: "gray_dark",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.43, 0.02, 0.02],
      size: [0.14, 0.56, 0.78],
      material: "gray_dark",
      faces: { [side === "l" ? "west" : "east"]: { texture: "wing_bars" } },
      joint: { pivot: [sign * -0.05, 0.18, -0.18], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [sign * 0.08, -0.08, 0.2],
      size: [0.08, 0.42, 0.5],
      material: "black",
    }));
  }

  part("neck_lower", box({
    parent: "body",
    at: [0, 0.48, -0.38],
    rot: [-10, 0, 0],
    size: [0.3, 0.48, 0.32],
    material: "gray_light",
    joint: { pivot: [0, -0.2, 0.08], axis: [1, 0, 0] },
  }));
  part("neck_upper", box({
    parent: "neck_lower",
    at: [0, 0.36, -0.1],
    rot: [8, 0, 0],
    size: [0.28, 0.4, 0.28],
    material: "gray_light",
  }));
  part("head", box({
    parent: "neck_upper",
    at: [0, 0.28, -0.14],
    size: [0.48, 0.4, 0.44],
    material: "gray_light",
    faces: { north: { texture: "face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.06, -0.36],
    size: [0.34, 0.2, 0.32],
    material: "beak",
  }));
  part("beak_tip", box({
    parent: "beak",
    at: [0, -0.04, -0.2],
    size: [0.25, 0.14, 0.14],
    material: "beak_tip",
  }));

  for (const [index, x, z, height, tilt] of [
    [1, -0.18, 0.08, 0.28, -10],
    [2, -0.09, 0.12, 0.34, -6],
    [3, 0, 0.14, 0.38, 0],
    [4, 0.09, 0.12, 0.34, 6],
    [5, 0.18, 0.08, 0.28, 10],
  ] as const) {
    part(`crest_${index}`, box({
      parent: "head",
      at: [x, 0.27, z],
      rot: [-24, 0, tilt],
      size: [0.065, height, 0.09],
      material: index === 3 ? "black_light" : "black",
      joint: { pivot: [0, -height * 0.42, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [side, x] of [["l", -0.23], ["r", 0.23]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.62, 0.06],
      size: [0.18, 0.6, 0.2],
      material: "black_light",
      joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.5, 0.02],
      size: [0.13, 0.56, 0.14],
      material: "leg",
      joint: { pivot: [0, 0.26, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `shin_${side}`,
      at: [0, -0.33, -0.2],
      size: [0.34, 0.14, 0.58],
      material: "foot",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  for (const [index, x] of [[1, -0.18], [2, 0], [3, 0.18]] as const) {
    part(`tail_feather_${index}`, box({
      parent: "rump",
      at: [x, 0, 0.3],
      rot: [12, 0, x * 12],
      size: [0.16, 0.12, 0.52],
      material: index === 2 ? "black" : "black_light",
      joint: { pivot: [0, 0, -0.22], axis: [1, 0, 0] },
    }));
  }

  bipedWalk("stalk", {
    label: "Grassland stalk",
    fps: 20,
    duration: 1.08,
    cycleDistance: 0.72,
    loop: true,
    samples: 23,
    body: "body",
    bodyBob: 0.015,
    bodyBobCenter: 0.018,
    head: "head",
    headSwingDegrees: 2.4,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.68,
    swingDegrees: 18,
    tracks: [
      swing("wing_l", { axis: "z", degrees: 3.5, center: -1.5, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -3.5, center: 1.5, frequency: 2 }),
      followThrough("neck_lower", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.5, lag: 0.11 }),
      followThrough("crest_2", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.65, lag: 0.13 }),
      followThrough("crest_3", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.15 }),
      followThrough("crest_4", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.65, lag: 0.13 }),
    ],
  });
  clip("stomp_strike", {
    label: "Stomp strike",
    role: "action",
    nextClip: "stalk",
    fps: 30,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0] }],
      ["body", 0.22, { at: [0, 0.035, 0] }],
      ["body", 0.46, { at: [0, 0.015, 0] }],
      ["body", 0.58, { at: [0, 0, 0] }],
      ["body", 0.78, { at: [0, 0, 0] }],
      ["neck_lower", 0, { rot: [0, 0, 0] }],
      ["neck_lower", 0.28, { rot: [-10, 0, 0] }],
      ["neck_lower", 0.5, { rot: [8, 0, 0] }],
      ["neck_lower", 0.78, { rot: [0, 0, 0] }],
      ["leg_l", 0, { rot: [0, 0, 0] }],
      ["leg_l", 0.2, { rot: [48, 0, 0] }],
      ["leg_l", 0.4, { rot: [58, 0, 0] }],
      ["leg_l", 0.5, { rot: [10, 0, 0] }],
      ["leg_l", 0.58, { rot: [0, 0, 0] }],
      ["leg_l", 0.78, { rot: [0, 0, 0] }],
      ["shin_l", 0, { rot: [0, 0, 0] }],
      ["shin_l", 0.2, { rot: [-38, 0, 0] }],
      ["shin_l", 0.4, { rot: [-20, 0, 0] }],
      ["shin_l", 0.5, { rot: [-4, 0, 0] }],
      ["shin_l", 0.58, { rot: [0, 0, 0] }],
      ["shin_l", 0.78, { rot: [0, 0, 0] }],
      ["wing_l", 0, { rot: [0, 0, 0] }],
      ["wing_l", 0.28, { rot: [0, 0, -14] }],
      ["wing_l", 0.54, { rot: [0, 0, -5] }],
      ["wing_l", 0.78, { rot: [0, 0, 0] }],
      ["wing_r", 0, { rot: [0, 0, 0] }],
      ["wing_r", 0.28, { rot: [0, 0, 14] }],
      ["wing_r", 0.54, { rot: [0, 0, 5] }],
      ["wing_r", 0.78, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("stalk");
});
