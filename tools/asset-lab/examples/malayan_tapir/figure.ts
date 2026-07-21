import { figure } from "../../src/dsl";

// A box-only Malayan tapir with a massive pale saddle between black fore and
// rear masses, white-rimmed ears, broad feet, and a short three-stage trunk.
export default figure("malayan_tapir", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("black", "#242524");
  mat("black_light", "#3c3d3b");
  mat("white", "#deddd4");
  mat("white_shadow", "#bdbdb6");
  mat("muzzle", "#4e4c48");
  mat("nose", "#171716");
  mat("hoof", "#1e1e1c");
  mat("ear_inner", "#7a5b5c");

  asciiTexture("face", {
    palette: { ".": "#242524", "l": "#3c3d3b", "e": "#b79445", "w": "#deddd4" },
    pixels: [
      "ww....ww",
      "l.e..e.l",
      "..e..e..",
      "...ll...",
      "..llll..",
      "........",
    ],
  });
  asciiTexture("trunk_face", {
    palette: { ".": "#4e4c48", "n": "#171716", "l": "#77736c" },
    pixels: ["..llll..", ".llllll.", ".nn..nn.", "..nnnn..", "........"],
  });
  asciiTexture("saddle_side", {
    palette: { ".": "#deddd4", "s": "#bdbdb6", "b": "#242524" },
    pixels: [
      "bbb........bbb",
      "bb..........bb",
      "b............b",
      "..............",
      "s............s",
      "ss..........ss",
      "bbb........bbb",
    ],
  });
  asciiTexture("hoof_face", {
    palette: { "h": "#1e1e1c", "c": "#0e0e0d" },
    pixels: ["hhhhhh", "hhcchh", "cccccc"],
  });

  part("body", box({
    at: [0, 1.02, 0.12],
    size: [1.04, 0.82, 1.58],
    material: "white",
    faces: {
      east: { texture: "saddle_side" },
      west: { texture: "saddle_side" },
    },
  }));
  part("saddle_top", box({
    parent: "body",
    at: [0, 0.46, 0.16],
    size: [0.92, 0.16, 1.08],
    material: "white_shadow",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.04, -0.65],
    size: [1.1, 0.82, 0.5],
    material: "black",
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.05, 0.68],
    size: [1.02, 0.72, 0.52],
    material: "black_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.46, 0.08],
    size: [0.9, 0.14, 1.18],
    material: "black_light",
  }));
  part("neck", box({
    parent: "shoulders",
    at: [0, -0.02, -0.43],
    rot: [10, 0, 0],
    size: [0.76, 0.66, 0.46],
    material: "black",
    joint: { pivot: [0, 0, 0.2], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, -0.04, -0.43],
    rot: [5, 0, 0],
    size: [0.64, 0.56, 0.54],
    material: "black_light",
    faces: { north: { texture: "face" } },
  }));
  part("snout_base", box({
    parent: "head",
    at: [0, -0.11, -0.36],
    rot: [-3, 0, 0],
    size: [0.46, 0.3, 0.28],
    material: "muzzle",
  }));
  part("snout_mid", box({
    parent: "snout_base",
    at: [0, -0.04, -0.21],
    rot: [-8, 0, 0],
    size: [0.34, 0.24, 0.2],
    material: "muzzle",
  }));
  part("snout_tip", box({
    parent: "snout_mid",
    at: [0, -0.04, -0.14],
    rot: [-6, 0, 0],
    size: [0.3, 0.21, 0.14],
    material: "nose",
    faces: { north: { texture: "trunk_face" } },
  }));
  for (const [side, x] of [["l", -0.25], ["r", 0.25]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.34, 0.08],
      rot: [-3, 0, side === "l" ? -12 : 12],
      size: [0.2, 0.25, 0.13],
      material: "ear_inner",
      joint: { pivot: [0, -0.11, 0], axis: [1, 0, 0] },
    }));
    part(`ear_tip_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0.16, 0],
      size: [0.17, 0.1, 0.11],
      material: "white",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.36, -0.5],
    ["fr", 0.36, -0.5],
    ["bl", -0.36, 0.52],
    ["br", 0.36, 0.52],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.61, z],
      size: [0.25, 0.5, 0.27],
      material: "black",
      joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.32, -0.05],
      size: [0.34, 0.14, 0.4],
      material: "hoof",
      faces: { north: { texture: "hoof_face" } },
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.06, 0.33],
    rot: [-16, 0, 0],
    size: [0.13, 0.3, 0.13],
    material: "black",
    joint: { pivot: [0, 0.13, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.24,
    cycleDistance: 0.72,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.015,
    bodyBobCenter: 0.017,
    head: "neck",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.71,
    swingDegrees: 15,
    tail: "tail",
    tailSwingDegrees: 5,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.12 }),
      followThrough("snout_mid", { source: "neck", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 3, overshoot: 0.4, lag: 0.11 }),
    ],
  });
});
