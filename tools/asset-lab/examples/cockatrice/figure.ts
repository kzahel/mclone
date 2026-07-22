import { figure } from "../../src/dsl";

// A ground-dwelling cockatrice built entirely from fused bird and reptile
// anatomy: rooster head and breast, compact wings, scaled legs, clawed feet,
// and a long articulated lizard tail.
export default figure("cockatrice", ({
  asciiTexture,
  bipedWalk,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
  swing,
}) => {
  metadata({
    bodyPlans: ["biped", "winged"],
    disposition: "neutral",
    groups: ["animal", "fantasy"],
    habitats: ["land"],
    scale: "medium",
    themes: ["cockatrice", "mythic", "reptilian", "rooster", "scaled"],
  });

  mat("plumage", "#74512e");
  mat("plumage_light", "#a7793f");
  mat("plumage_dark", "#352a21");
  mat("scale", "#657247");
  mat("scale_light", "#89945f");
  mat("scale_dark", "#3f4c35");
  mat("comb", "#a84135");
  mat("comb_dark", "#6d2d29");
  mat("beak", "#d3a448");
  mat("eye", "#d9b83d");
  mat("claw", "#28231d");

  asciiTexture("cockatrice_face", {
    palette: { ".": "#74512e", "d": "#352a21", "e": "#d9b83d", "p": "#171713", "s": "#657247" },
    pixels: [
      "dd......dd",
      "d.ep..pe.d",
      "..ep..pe..",
      "....ss....",
      "...ssss...",
      "..d....d..",
      "..........",
      "dd......dd",
    ],
  });
  asciiTexture("breast_scales", {
    palette: { ".": "#a7793f", "d": "#352a21", "s": "#657247", "l": "#89945f" },
    pixels: [
      "ddssssssdd",
      "dssllllssd",
      "ssllssllss",
      "sllsssslls",
      "ssllssllss",
      "dssllllssd",
      "ddssssssdd",
    ],
  });
  asciiTexture("wing_bands", {
    palette: { ".": "#74512e", "l": "#a7793f", "d": "#352a21", "s": "#657247" },
    pixels: ["dddddddd", "dllllddd", "d....lld", "dsssssdd", "d....ddd", "dddddddd"],
  });
  asciiTexture("tail_scales", {
    palette: { ".": "#657247", "l": "#89945f", "d": "#3f4c35" },
    pixels: ["llddlldd", "ddllddll", "llddlldd", "dddddddd"],
  });
  asciiTexture("clawed_foot", {
    palette: { ".": "#657247", "c": "#28231d" },
    pixels: ["........", ".c.cc.c.", "cccccccc"],
  });

  part("body", box({
    at: [0, 0.88, 0.04],
    size: [0.78, 0.7, 0.88],
    material: "plumage",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.02, -0.5],
    size: [0.62, 0.58, 0.2],
    material: "plumage_light",
    faces: { north: { texture: "breast_scales" } },
  }));
  part("back_scales", box({
    parent: "body",
    at: [0, 0.38, 0.08],
    size: [0.56, 0.18, 0.62],
    material: "scale_dark",
    faces: { up: { texture: "tail_scales" } },
  }));

  part("neck", box({
    parent: "body",
    at: [0, 0.48, -0.34],
    rot: [-5, 0, 0],
    size: [0.32, 0.62, 0.32],
    material: "scale",
    joint: { pivot: [0, -0.28, 0.08], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.42, -0.08],
    size: [0.5, 0.42, 0.44],
    material: "plumage",
    faces: { north: { texture: "cockatrice_face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.06, -0.31],
    size: [0.3, 0.18, 0.22],
    material: "beak",
  }));
  part("wattle", box({
    parent: "head",
    at: [0, -0.28, -0.12],
    size: [0.18, 0.2, 0.12],
    material: "comb_dark",
    joint: { pivot: [0, 0.09, 0], axis: [1, 0, 0] },
  }));
  for (const [index, z, height] of [
    [1, -0.08, 0.26],
    [2, 0.06, 0.3],
    [3, 0.19, 0.24],
  ] as const) {
    part(`comb_${index}`, box({
      parent: "head",
      at: [0, 0.29, z],
      rot: [index === 1 ? -5 : index === 3 ? 5 : 0, 0, 0],
      size: [index === 2 ? 0.14 : 0.12, height, 0.13],
      material: index === 2 ? "comb" : "comb_dark",
    }));
  }

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.43, 0.02, 0],
      rot: [0, sign * -5, sign * 4],
      size: [0.16, 0.56, 0.64],
      material: side === "l" ? "plumage" : "plumage_light",
      faces: { east: { texture: "wing_bands" }, west: { texture: "wing_bands" } },
      joint: { pivot: [sign * -0.06, 0.23, -0.18], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [sign * 0.04, -0.31, 0.06],
      size: [0.14, 0.28, 0.5],
      material: "plumage_dark",
      faces: { east: { texture: "wing_bands" }, west: { texture: "wing_bands" } },
    }));
    part(`leg_${side}`, box({
      parent: "body",
      at: [sign * 0.21, -0.58, -0.02],
      size: [0.15, 0.48, 0.16],
      material: "scale_light",
      faces: { north: { texture: "tail_scales" }, south: { texture: "tail_scales" } },
      joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.235, -0.11],
      size: [0.32, 0.13, 0.46],
      material: "scale",
      faces: { north: { texture: "clawed_foot" } },
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.04, 0.57],
    size: [0.4, 0.3, 0.58],
    material: "scale",
    faces: { up: { texture: "tail_scales" }, east: { texture: "tail_scales" }, west: { texture: "tail_scales" } },
    joint: { pivot: [0, 0, -0.26], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.05, 0.5],
    size: [0.3, 0.24, 0.52],
    material: "scale_light",
    faces: { up: { texture: "tail_scales" } },
    joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] },
  }));
  part("tail_3", box({
    parent: "tail_2",
    at: [0, -0.04, 0.45],
    size: [0.22, 0.18, 0.46],
    material: "scale_dark",
    faces: { up: { texture: "tail_scales" } },
    joint: { pivot: [0, 0, -0.2], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_3",
    at: [0, 0, 0.39],
    size: [0.14, 0.1, 0.4],
    material: "scale",
    joint: { pivot: [0, 0, -0.18], axis: [0, 1, 0] },
  }));

  bipedWalk("scaled_strut", {
    label: "Scaled strut",
    fps: 20,
    duration: 1.02,
    cycleDistance: 0.48,
    loop: true,
    samples: 23,
    armSwingDegrees: 5,
    body: "body",
    bodyBob: 0.015,
    bodyBobCenter: 0.017,
    head: "head",
    headSwingDegrees: 3,
    leftArm: "wing_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "wing_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.68,
    swingDegrees: 17,
    tracks: [
      swing("tail_1", { axis: "y", degrees: 6, phase: 0.05 }),
      swing("tail_2", { axis: "y", degrees: 10, phase: 0.16 }),
      swing("tail_3", { axis: "y", degrees: 13, phase: 0.27 }),
      swing("tail_tip", { axis: "y", degrees: 16, phase: 0.38 }),
      followThrough("wattle", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.62, lag: 0.14 }),
    ],
  });
  clip("threat_display", {
    label: "Threat display",
    role: "action",
    nextClip: "scaled_strut",
    fps: 30,
    loop: false,
    keys: [
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.24, { rot: [-13, 0, 0] }],
      ["neck", 0.62, { rot: [-17, 0, 0] }],
      ["neck", 1.16, { rot: [0, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.24, { rot: [7, -7, 0] }],
      ["head", 0.62, { rot: [10, 8, 0] }],
      ["head", 1.16, { rot: [0, 0, 0] }],
      ["wing_l", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_l", 0.24, { at: [-0.06, 0.05, 0], rot: [0, 0, -36] }],
      ["wing_l", 0.62, { at: [-0.08, 0.08, 0], rot: [0, 0, -48] }],
      ["wing_l", 1.16, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_r", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["wing_r", 0.24, { at: [0.06, 0.05, 0], rot: [0, 0, 36] }],
      ["wing_r", 0.62, { at: [0.08, 0.08, 0], rot: [0, 0, 48] }],
      ["wing_r", 1.16, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["tail_1", 0, { rot: [0, 0, 0] }],
      ["tail_1", 0.24, { rot: [0, -10, 0] }],
      ["tail_1", 0.62, { rot: [0, 14, 0] }],
      ["tail_1", 1.16, { rot: [0, 0, 0] }],
      ["tail_2", 0, { rot: [0, 0, 0] }],
      ["tail_2", 0.24, { rot: [0, -16, 0] }],
      ["tail_2", 0.62, { rot: [0, 22, 0] }],
      ["tail_2", 1.16, { rot: [0, 0, 0] }],
      ["tail_3", 0, { rot: [0, 0, 0] }],
      ["tail_3", 0.24, { rot: [0, -20, 0] }],
      ["tail_3", 0.62, { rot: [0, 28, 0] }],
      ["tail_3", 1.16, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("scaled_strut");
});
