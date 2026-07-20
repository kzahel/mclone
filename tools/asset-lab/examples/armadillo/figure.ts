import { figure } from "../../src/dsl";

// A box-only nine-banded armadillo with four stepped armor bands, pointed
// head, upright ears, broad clawed feet, and a tapered three-stage plated tail.
// The shell remains rigid while the low legs and tail carry the scuttle cycle.
export default figure("armadillo", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  swing,
  followThrough,
}) => {
  mat("hide", "#806d5c");
  mat("hide_light", "#9d8873");
  mat("hide_dark", "#554940");
  mat("armor", "#857665");
  mat("armor_light", "#a3937d");
  mat("armor_dark", "#655b50");
  mat("belly", "#a98f77");
  mat("muzzle", "#9a806b");
  mat("nose", "#24201d");
  mat("paw", "#66574b");
  mat("claw", "#ddd0ad");

  asciiTexture("shell_side", {
    palette: { ".": "#857665", "l": "#a3937d", "d": "#655b50", "h": "#554940" },
    pixels: [
      "dddddddddddd",
      "dlllllllllld",
      "l..h..h..h.l",
      "..h..h..h...",
      ".h..h..h..h.",
      "hhhhhhhhhhhh",
    ],
  });
  asciiTexture("shell_top", {
    palette: { ".": "#857665", "l": "#a3937d", "d": "#655b50" },
    pixels: [
      "dddddddddd",
      "dlllllllll",
      "dl..d..d.l",
      "d..d..d..l",
      "d.d..d..dl",
      "dddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#9d8873", "e": "#171513", "d": "#554940", "l": "#b49d85" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      "..llll..",
      ".llllll.",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#9a806b", "n": "#24201d" },
    pixels: [
      "........",
      "..nnnn..",
      ".nnnnnn.",
      "...nn...",
    ],
  });
  asciiTexture("claw_front", {
    palette: { ".": "#66574b", "c": "#ddd0ad" },
    pixels: [
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.57, 0.07],
    size: [0.82, 0.46, 1.18],
    material: "hide",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.28, -0.02],
    size: [0.62, 0.11, 0.82],
    material: "belly",
  }));
  for (const [index, z, y, width, height, material] of [
    [1, -0.42, 0.15, 0.8, 0.42, "armor_light"],
    [2, -0.14, 0.21, 0.88, 0.52, "armor"],
    [3, 0.17, 0.2, 0.88, 0.5, "armor_dark"],
    [4, 0.46, 0.14, 0.8, 0.4, "armor"],
  ] as const) {
    part(`shell_band_${index}`, box({
      parent: "body",
      at: [0, y, z],
      size: [width, height, 0.3],
      material,
      faces: {
        east: { texture: "shell_side" },
        west: { texture: "shell_side" },
        up: { texture: "shell_top" },
      },
    }));
  }

  part("head", box({
    parent: "body",
    at: [0, -0.02, -0.77],
    size: [0.48, 0.4, 0.44],
    material: "hide_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.18], axis: [1, 0, 0] },
  }));
  part("head_plate", box({
    parent: "head",
    at: [0, 0.22, 0.02],
    size: [0.4, 0.12, 0.34],
    material: "armor_light",
    faces: { up: { texture: "shell_top" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.34],
    size: [0.32, 0.22, 0.28],
    material: "muzzle",
  }));
  part("muzzle_tip", box({
    parent: "muzzle",
    at: [0, -0.04, -0.2],
    size: [0.22, 0.15, 0.16],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.18], ["r", 0.18]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.3, 0.08],
      rot: [0, 0, side === "l" ? -7 : 7],
      size: [0.13, 0.24, 0.11],
      material: "hide_dark",
      joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.3, -0.3],
    ["fr", 0.3, -0.3],
    ["bl", -0.3, 0.34],
    ["br", 0.3, 0.34],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.34, z],
      size: [0.15, 0.29, 0.17],
      material: "hide_dark",
      joint: { pivot: [0, 0.145, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.18, -0.075],
      size: [0.3, 0.1, 0.34],
      material: "paw",
      faces: { north: { texture: "claw_front" } },
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, -0.02, 0.78],
    size: [0.34, 0.28, 0.5],
    material: "armor",
    faces: { up: { texture: "shell_top" } },
    joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.02, 0.41],
    size: [0.24, 0.2, 0.4],
    material: "armor_light",
    faces: { up: { texture: "shell_top" } },
    joint: { pivot: [0, 0, -0.18], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, -0.02, 0.31],
    size: [0.14, 0.13, 0.3],
    material: "armor_dark",
  }));

  quadrupedWalk("scuttle", {
    fps: 20,
    duration: 0.88,
    cycleDistance: 0.56,
    gait: "trot",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.01,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.6,
    swingDegrees: 18,
    tracks: [
      swing("tail_1", { axis: "y", degrees: 8, phase: 0.5 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 11, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.1 }),
    ],
  });
});
