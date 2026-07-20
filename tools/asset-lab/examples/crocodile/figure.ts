import { figure } from "../../src/dsl";

// A box-only adult Nile crocodile with a low plated torso, long two-level jaw,
// high-set eyes, sprawled feet, and a tapering articulated tail. The walk is
// intentionally slow and shallow so the belly remains close to the ground.
export default figure("crocodile", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("hide", "#4f6535");
  mat("hide_light", "#71834b");
  mat("hide_dark", "#2d4228");
  mat("belly", "#a39b68");
  mat("scute", "#354c2b");
  mat("eye", "#d6bb55");
  mat("pupil", "#17160f");
  mat("mouth", "#4d2828");
  mat("tooth", "#eee5c7");
  mat("claw", "#d3c699");

  asciiTexture("body_side", {
    palette: { ".": "#4f6535", "d": "#2d4228", "l": "#71834b", "b": "#a39b68" },
    pixels: [
      "dddddddddddddd",
      "d.ll..ll..ll.d",
      "l............l",
      "..............",
      "..bbbbbbbbbb..",
      ".bbbbbbbbbbbb.",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#71834b", "n": "#1f2a1c" },
    pixels: [
      ".nn....nn.",
      ".nn....nn.",
      "..........",
      "..........",
      "..........",
    ],
  });
  asciiTexture("jaw_side", {
    palette: { "m": "#4d2828", "t": "#eee5c7", "h": "#4f6535" },
    pixels: [
      "hhhhhhhhhhhh",
      "tttttttttttt",
      "mmmmmmmmmmmm",
      "tttttttttttt",
      "hhhhhhhhhhhh",
    ],
  });
  asciiTexture("foot_front", {
    palette: { ".": "#2d4228", "c": "#d3c699" },
    pixels: [
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.62, 0.12],
    size: [1.02, 0.48, 1.66],
    material: "hide",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.29, -0.02],
    size: [0.86, 0.12, 1.38],
    material: "belly",
  }));
  for (const [index, z, width] of [
    [1, -0.5, 0.72],
    [2, 0.02, 0.8],
    [3, 0.54, 0.68],
  ] as const) {
    part(`back_scute_${index}`, box({
      parent: "body",
      at: [0, 0.29, z],
      size: [width, 0.14, 0.34],
      material: "scute",
    }));
  }

  part("neck", box({
    parent: "body",
    at: [0, -0.01, -0.93],
    size: [0.9, 0.4, 0.4],
    material: "hide_light",
    joint: { pivot: [0, 0, 0.18], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.02, -0.4],
    size: [0.84, 0.4, 0.54],
    material: "hide_light",
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.04, -0.47],
    size: [0.78, 0.3, 0.46],
    material: "hide_light",
    faces: { north: { texture: "snout_face" } },
  }));
  part("jaw", box({
    parent: "head",
    at: [0, -0.24, -0.26],
    size: [0.74, 0.13, 0.78],
    material: "mouth",
    faces: {
      east: { texture: "jaw_side" },
      west: { texture: "jaw_side" },
    },
  }));
  for (const [side, x] of [["l", -0.31], ["r", 0.31]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [x, 0.25, -0.08],
      size: [0.22, 0.18, 0.24],
      material: "hide_dark",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0.015, -0.135],
      size: [0.09, 0.09, 0.035],
      material: "eye",
      faces: { north: { material: "pupil" } },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.62, -0.5],
    ["fr", 0.62, -0.5],
    ["bl", -0.62, 0.5],
    ["br", 0.62, 0.5],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.28, z],
      size: [0.42, 0.2, 0.34],
      material: "hide_dark",
      joint: { pivot: [x < 0 ? 0.18 : -0.18, 0.06, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [x < 0 ? -0.26 : 0.26, -0.15, -0.04],
      size: [0.42, 0.13, 0.46],
      material: "hide_dark",
      faces: { north: { texture: "foot_front" } },
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.01, 1.05],
    size: [0.66, 0.4, 0.66],
    material: "hide",
    joint: { pivot: [0, 0, -0.31], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.03, 0.56],
    size: [0.44, 0.3, 0.64],
    material: "hide_light",
    joint: { pivot: [0, 0, -0.3], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, -0.02, 0.48],
    size: [0.22, 0.2, 0.5],
    material: "hide_dark",
  }));

  quadrupedWalk("sprawl_walk", {
    fps: 18,
    duration: 1.62,
    cycleDistance: 0.58,
    gait: "walk",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.008,
    bodyBobCenter: 0.01,
    head: "neck",
    headSwingDegrees: 1.6,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.78,
    swingDegrees: 9,
    tail: "tail_1",
    tailSwingDegrees: 5,
    tracks: [
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 8, overshoot: 0.3, lag: 0.1 }),
    ],
  });
});
