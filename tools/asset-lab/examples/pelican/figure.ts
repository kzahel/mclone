import { figure } from "../../src/dsl";

// A box-only adult great white pelican with broad white wings, a long neck,
// enormous orange bill and pouch, planted webbed feet, and a scoop action.
export default figure("pelican", ({
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
  mat("white", "#e6e3d5");
  mat("white_light", "#f2efe3");
  mat("white_shadow", "#bfc2b8");
  mat("flight", "#343a3a");
  mat("face", "#d3b083");
  mat("bill", "#d99a48");
  mat("bill_light", "#efbd67");
  mat("pouch", "#ca7650");
  mat("eye", "#171918");
  mat("leg", "#d49a62");
  mat("foot", "#b9794d");

  asciiTexture("wing_feathers", {
    palette: { ".": "#e6e3d5", "s": "#bfc2b8", "b": "#343a3a" },
    pixels: [
      "ssssssssssss",
      "s..........s",
      "..bbbbbbbb..",
      ".bbbbbbbbbb.",
      "bbbbbbbbbbbb",
      "bbbbbbbbbbbb",
    ],
  });
  asciiTexture("face_mask", {
    palette: { ".": "#e6e3d5", "f": "#d3b083", "e": "#171918" },
    pixels: ["........", ".ff..ff.", ".fe..ef.", "..ffff..", "........"],
  });
  asciiTexture("bill_top", {
    palette: { ".": "#d99a48", "l": "#efbd67", "d": "#9c6038" },
    pixels: ["llllllllllll", "l..........l", "............", "..dddddddd.."],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#b9794d", "d": "#75452f" },
    pixels: ["...dd...", "..dddd..", "dddddddd", "dd....dd"],
  });

  part("body", box({
    at: [0, 0.88, 0.08],
    size: [0.9, 0.68, 1.22],
    material: "white",
  }));
  part("breast", box({
    parent: "body",
    at: [0, 0, -0.65],
    size: [0.72, 0.58, 0.2],
    material: "white_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.03, 0.64],
    size: [0.76, 0.54, 0.2],
    material: "white_shadow",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.51, 0.03, 0.06],
      size: [0.14, 0.5, 0.92],
      material: "white_shadow",
      faces: { [side === "l" ? "west" : "east"]: { texture: "wing_feathers" } },
      joint: { pivot: [sign * -0.055, 0.16, -0.28], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [sign * 0.075, -0.07, 0.22],
      size: [0.07, 0.34, 0.52],
      material: "flight",
    }));
  }

  part("neck_lower", box({
    parent: "body",
    at: [0, 0.48, -0.42],
    rot: [-12, 0, 0],
    size: [0.34, 0.5, 0.36],
    material: "white_light",
    joint: { pivot: [0, -0.22, 0.1], axis: [1, 0, 0] },
  }));
  part("neck_upper", box({
    parent: "neck_lower",
    at: [0, 0.38, -0.08],
    rot: [10, 0, 0],
    size: [0.3, 0.42, 0.32],
    material: "white_light",
    joint: { pivot: [0, -0.18, 0.08], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck_upper",
    at: [0, 0.27, -0.13],
    size: [0.48, 0.38, 0.44],
    material: "face",
    faces: { north: { texture: "face_mask" } },
  }));
  part("upper_bill", box({
    parent: "head",
    at: [0, -0.05, -0.55],
    size: [0.4, 0.15, 0.82],
    material: "bill",
    faces: { up: { texture: "bill_top" } },
  }));
  part("bill_tip", box({
    parent: "upper_bill",
    at: [0, -0.01, -0.45],
    size: [0.32, 0.12, 0.14],
    material: "bill_light",
  }));
  part("pouch", box({
    parent: "head",
    at: [0, -0.18, -0.49],
    rot: [3, 0, 0],
    size: [0.34, 0.2, 0.68],
    material: "pouch",
    joint: { pivot: [0, 0.07, 0.27], axis: [1, 0, 0] },
  }));

  for (const [side, x] of [["l", -0.24], ["r", 0.24]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.44, 0.08],
      size: [0.13, 0.3, 0.15],
      material: "leg",
      joint: { pivot: [0, 0.14, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.24, -0.14],
      size: [0.42, 0.12, 0.5],
      material: "foot",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.08, 0.27],
    rot: [20, 0, 0],
    size: [0.56, 0.16, 0.46],
    material: "flight",
    joint: { pivot: [0, 0, -0.2], axis: [1, 0, 0] },
  }));

  bipedWalk("shore_waddle", {
    label: "Shore waddle",
    fps: 18,
    duration: 1.02,
    cycleDistance: 0.5,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.018,
    bodyBobCenter: 0.02,
    head: "head",
    headSwingDegrees: 2.4,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.7,
    swingDegrees: 15,
    tracks: [
      swing("body", { axis: "z", degrees: 3.8, phase: 0.25 }),
      swing("wing_l", { axis: "z", degrees: 4, center: -2, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -4, center: 2, frequency: 2 }),
      followThrough("neck_lower", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4.5, overshoot: 0.5, lag: 0.11 }),
      followThrough("pouch", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.6, lag: 0.14 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.55, lag: 0.13 }),
    ],
  });
  clip("pouch_scoop", {
    label: "Pouch scoop",
    role: "action",
    nextClip: "shore_waddle",
    fps: 30,
    loop: false,
    keys: [
      ["neck_lower", 0, { rot: [0, 0, 0] }],
      ["neck_lower", 0.32, { rot: [24, 0, 0] }],
      ["neck_lower", 0.58, { rot: [32, 0, 0] }],
      ["neck_lower", 0.9, { rot: [10, 0, 0] }],
      ["neck_lower", 1.16, { rot: [0, 0, 0] }],
      ["neck_upper", 0, { rot: [0, 0, 0] }],
      ["neck_upper", 0.32, { rot: [-28, 0, 0] }],
      ["neck_upper", 0.58, { rot: [-38, 0, 0] }],
      ["neck_upper", 0.9, { rot: [-12, 0, 0] }],
      ["neck_upper", 1.16, { rot: [0, 0, 0] }],
      ["pouch", 0, { rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["pouch", 0.38, { rot: [-12, 0, 0], scale: [1.04, 1.28, 1.08] }],
      ["pouch", 0.62, { rot: [-20, 0, 0], scale: [1.08, 1.55, 1.12] }],
      ["pouch", 0.9, { rot: [-6, 0, 0], scale: [1.03, 1.18, 1.04] }],
      ["pouch", 1.16, { rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["wing_l", 0, { rot: [0, 0, 0] }],
      ["wing_l", 0.58, { rot: [0, 0, -9] }],
      ["wing_l", 1.16, { rot: [0, 0, 0] }],
      ["wing_r", 0, { rot: [0, 0, 0] }],
      ["wing_r", 0.58, { rot: [0, 0, 9] }],
      ["wing_r", 1.16, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("shore_waddle");
});
