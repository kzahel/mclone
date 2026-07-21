import { figure } from "../../src/dsl";

// A box-only shoebill with slate-gray plumage, a broad hooked bill, short
// crest, long grounded legs, and a deliberately abrupt bill-snap action.
export default figure("shoebill", ({
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
  mat("slate", "#697276");
  mat("slate_light", "#8a9496");
  mat("slate_dark", "#454d50");
  mat("feather_dark", "#333a3d");
  mat("bill", "#a79d7a");
  mat("bill_light", "#c4b995");
  mat("bill_tip", "#5e5a49");
  mat("eye", "#d7b84d");
  mat("leg", "#77705d");
  mat("foot", "#48453c");

  asciiTexture("face", {
    palette: { ".": "#8a9496", "d": "#454d50", "e": "#d7b84d", "b": "#171817" },
    pixels: ["dddddddd", "d.e..e.d", "d.b..b.d", "d......d", ".d....d.", "..dddd.."],
  });
  asciiTexture("wing_feathers", {
    palette: { ".": "#697276", "l": "#8a9496", "d": "#454d50", "b": "#333a3d" },
    pixels: ["llllllllll", "l........l", "..dd..dd..", ".d..dd..d.", "bbbbbbbbbb", "b........b"],
  });
  asciiTexture("bill_side", {
    palette: { ".": "#a79d7a", "l": "#c4b995", "d": "#5e5a49" },
    pixels: ["llllllllllll", "l..........d", "..ll.......d", "....llll...d", "..........dd", "dddddddddddd"],
  });

  part("body", box({
    at: [0, 1.56, 0.08],
    size: [0.74, 0.74, 1.02],
    material: "slate",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.02, -0.55],
    size: [0.62, 0.62, 0.18],
    material: "slate_light",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.42, 0.02, 0.08],
      size: [0.12, 0.58, 0.78],
      material: "slate_dark",
      faces: { [side === "l" ? "west" : "east"]: { texture: "wing_feathers" } },
      joint: { pivot: [sign * -0.04, 0.22, -0.22], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [0, -0.26, 0.14],
      size: [0.13, 0.28, 0.52],
      material: "feather_dark",
    }));
  }
  part("neck_lower", box({
    parent: "body",
    at: [0, 0.62, -0.36],
    rot: [-8, 0, 0],
    size: [0.38, 0.7, 0.38],
    material: "slate_dark",
    joint: { pivot: [0, -0.33, 0.07], axis: [1, 0, 0] },
  }));
  part("neck_upper", box({
    parent: "neck_lower",
    at: [0, 0.46, -0.08],
    rot: [9, 0, 0],
    size: [0.34, 0.44, 0.34],
    material: "slate_light",
  }));
  part("head", box({
    parent: "neck_upper",
    at: [0, 0.35, -0.12],
    rot: [4, 0, 0],
    size: [0.56, 0.48, 0.5],
    material: "slate_light",
    faces: { north: { texture: "face" } },
  }));
  for (const [name, x, lean] of [["crest_l", -0.13, -9], ["crest_c", 0, 0], ["crest_r", 0.13, 9]] as const) {
    part(name, box({
      parent: "head",
      at: [x, 0.3, 0.12],
      rot: [-12, 0, lean],
      size: [0.09, name === "crest_c" ? 0.28 : 0.22, 0.08],
      material: "feather_dark",
    }));
  }
  part("upper_bill", box({
    parent: "head",
    at: [0, -0.02, -0.51],
    size: [0.54, 0.34, 0.62],
    material: "bill_light",
    faces: { east: { texture: "bill_side" }, west: { texture: "bill_side" } },
    joint: { pivot: [0, 0, 0.27], axis: [1, 0, 0] },
  }));
  part("bill_hook", box({
    parent: "upper_bill",
    at: [0, -0.05, -0.37],
    rot: [8, 0, 0],
    size: [0.42, 0.26, 0.18],
    material: "bill_tip",
  }));
  part("lower_bill", box({
    parent: "head",
    at: [0, -0.2, -0.49],
    size: [0.46, 0.14, 0.58],
    material: "bill",
    faces: { east: { texture: "bill_side" }, west: { texture: "bill_side" } },
    joint: { pivot: [0, 0, 0.25], axis: [1, 0, 0] },
  }));

  for (const [side, x] of [["l", -0.23], ["r", 0.23]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.68, 0.02],
      size: [0.16, 0.68, 0.17],
      material: "leg",
      joint: { pivot: [0, 0.33, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.49, 0.04],
      size: [0.13, 0.66, 0.14],
      material: "leg",
    }));
    part(`foot_${side}`, box({
      parent: `shin_${side}`,
      at: [0, -0.32, -0.22],
      size: [0.34, 0.14, 0.66],
      material: "foot",
    }));
  }
  part("tail", box({
    parent: "body",
    at: [0, 0.04, 0.62],
    rot: [8, 0, 0],
    size: [0.48, 0.12, 0.36],
    material: "feather_dark",
    joint: { pivot: [0, 0, -0.16], axis: [1, 0, 0] },
  }));

  bipedWalk("stalk", {
    label: "Stalk",
    fps: 20,
    duration: 1.08,
    cycleDistance: 0.78,
    loop: true,
    samples: 23,
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 1.4,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.68,
    swingDegrees: 18,
    tracks: [
      swing("wing_l", { axis: "z", degrees: 2.5, center: -1, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -2.5, center: 1, frequency: 2 }),
      followThrough("neck_lower", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3.5, overshoot: 0.4, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.5, lag: 0.15 }),
    ],
  });
  clip("bill_snap", {
    label: "Bill snap",
    role: "action",
    nextClip: "stalk",
    fps: 30,
    loop: false,
    keys: [
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.16, { rot: [-5, 0, 0] }],
      ["head", 0.23, { rot: [3, 0, 0] }],
      ["head", 0.42, { rot: [0, 0, 0] }],
      ["lower_bill", 0, { rot: [0, 0, 0] }],
      ["lower_bill", 0.12, { rot: [-18, 0, 0] }],
      ["lower_bill", 0.2, { rot: [-26, 0, 0] }],
      ["lower_bill", 0.235, { rot: [2, 0, 0] }],
      ["lower_bill", 0.29, { rot: [-1, 0, 0] }],
      ["lower_bill", 0.42, { rot: [0, 0, 0] }],
      ["upper_bill", 0, { rot: [0, 0, 0] }],
      ["upper_bill", 0.2, { rot: [-3, 0, 0] }],
      ["upper_bill", 0.235, { rot: [2, 0, 0] }],
      ["upper_bill", 0.42, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("stalk");
});
