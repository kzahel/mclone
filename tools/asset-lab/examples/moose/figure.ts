import { figure } from "../../src/dsl";

// A box-only bull moose with tall legs, high shoulders, a long muzzle, hanging
// throat bell, and a broad ten-box palmate antler rack.
export default figure("moose", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#5d3b27");
  mat("coat_light", "#795038");
  mat("coat_dark", "#34251d");
  mat("coat_shadow", "#493023");
  mat("cream", "#bda381");
  mat("muzzle", "#826b59");
  mat("hoof", "#211b18");
  mat("antler", "#b89b73");
  mat("antler_light", "#ccb58e");
  mat("ear_inner", "#9a705d");

  asciiTexture("face", {
    palette: { ".": "#5d3b27", "d": "#34251d", "e": "#15110e", "l": "#795038" },
    pixels: [
      "dd....dd",
      "d......d",
      ".e....e.",
      "..d..d..",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#826b59", "n": "#241b17", "l": "#bda381" },
    pixels: [
      "..llll..",
      ".llllll.",
      ".nn..nn.",
      "..nnnn..",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#5d3b27", "d": "#34251d", "l": "#795038", "c": "#bda381" },
    pixels: [
      "dddddddddddddd",
      "d............d",
      "..llllllllll..",
      "...llllllll...",
      "....cccccc....",
      "..............",
    ],
  });

  part("body", box({
    at: [0, 1.34, 0.06],
    size: [0.9, 0.72, 1.68],
    material: "coat",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("shoulder_hump", box({
    parent: "body",
    at: [0, 0.31, -0.55],
    rot: [-4, 0, 0],
    size: [0.98, 0.56, 0.66],
    material: "coat_dark",
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.18, -0.72],
    size: [0.7, 0.5, 0.26],
    material: "coat_shadow",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.38, 0.03],
    size: [0.68, 0.1, 1.14],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.34, -0.78],
    rot: [-31, 0, 0],
    size: [0.58, 0.84, 0.5],
    material: "coat_light",
    joint: { pivot: [0, -0.4, 0.1], axis: [1, 0, 0] },
  }));
  part("throat_bell", box({
    parent: "neck",
    at: [0, -0.15, -0.31],
    rot: [8, 0, 0],
    size: [0.2, 0.55, 0.12],
    material: "coat_dark",
    joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.51, -0.22],
    rot: [25, 0, 0],
    size: [0.56, 0.5, 0.68],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.15, -0.49],
    size: [0.48, 0.3, 0.4],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x, lean] of [
    ["l", -0.35, -18],
    ["r", 0.35, 18],
  ] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.18, 0.08],
      rot: [-5, 0, lean],
      size: [0.3, 0.17, 0.11],
      material: "ear_inner",
      joint: { pivot: [side === "l" ? 0.13 : -0.13, 0, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [side, x, lean] of [
    ["l", -0.21, -32],
    ["r", 0.21, 32],
  ] as const) {
    part(`antler_root_${side}`, box({
      parent: "head",
      at: [x, 0.37, 0.08],
      rot: [-14, 0, lean],
      size: [0.1, 0.48, 0.1],
      material: "antler",
      joint: { pivot: [0, -0.22, 0], axis: [0, 0, 1] },
    }));
    part(`antler_palm_${side}`, box({
      parent: "head",
      at: [side === "l" ? -0.44 : 0.44, 0.58, 0.06],
      rot: [-10, 0, side === "l" ? -8 : 8],
      size: [0.58, 0.34, 0.12],
      material: "antler_light",
    }));
    for (const [tineIndex, tineX, tineHeight, tineLean] of [
      [1, -0.19, 0.3, 10],
      [2, 0, 0.39, 0],
      [3, 0.19, 0.32, -10],
    ] as const) {
      part(`antler_tine_${side}_${tineIndex}`, box({
        parent: `antler_palm_${side}`,
        at: [tineX, 0.27, -0.01],
        rot: [-7, 0, side === "l" ? -tineLean : tineLean],
        size: [0.075, tineHeight, 0.075],
        material: "antler",
      }));
    }
  }

  for (const [suffix, x, z] of [
    ["fl", -0.3, -0.53],
    ["fr", 0.3, -0.53],
    ["bl", -0.31, 0.54],
    ["br", 0.31, 0.54],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.78, z],
      size: [0.17, 0.92, 0.18],
      material: suffix.startsWith("b") ? "coat_shadow" : "coat_dark",
      joint: { pivot: [0, 0.46, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.52, -0.03],
      size: [0.21, 0.12, 0.25],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.11, 0.88],
    rot: [35, 0, 0],
    size: [0.16, 0.25, 0.16],
    material: "coat_dark",
    joint: { pivot: [0, 0.11, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.16, -0.02],
    size: [0.13, 0.13, 0.13],
    material: "cream",
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.18,
    cycleDistance: 0.92,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.011,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.68,
    swingDegrees: 16,
    tail: "tail",
    tailSwingDegrees: 5,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.14 }),
      followThrough("throat_bell", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.18 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.11 }),
      followThrough("antler_root_l", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2, overshoot: 0.3, lag: 0.09 }),
      followThrough("antler_root_r", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2, overshoot: 0.3, lag: 0.09 }),
    ],
  });
});
