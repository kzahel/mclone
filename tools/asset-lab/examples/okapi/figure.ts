import { figure } from "../../src/dsl";

// A box-only okapi with a deep chestnut body, shorter giraffe-like neck, huge
// ears, small paired ossicones, dark face, and white-barred rump and legs.
export default figure("okapi", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#70412f");
  mat("coat_light", "#925b42");
  mat("coat_dark", "#332923");
  mat("black", "#1d1b19");
  mat("white", "#eee7d8");
  mat("muzzle", "#d2b591");
  mat("ossicone", "#5b3d2f");
  mat("hoof", "#211d1a");
  mat("ear_inner", "#a66e68");

  asciiTexture("face", {
    palette: { ".": "#332923", "l": "#925b42", "e": "#c5a24a", "w": "#eee7d8" },
    pixels: [
      "ll....ll",
      "l.e..e.l",
      "..e..e..",
      "w......w",
      ".w....w.",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#d2b591", "n": "#3e3027" },
    pixels: ["........", ".nn..nn.", "..nnnn..", "........", "........"],
  });
  asciiTexture("body_side", {
    palette: { ".": "#70412f", "l": "#925b42", "d": "#332923", "w": "#eee7d8" },
    pixels: [
      "ddd........ddd",
      "d..llllllll..d",
      "..llllllllll..",
      "...........www",
      ".........ww..w",
      "dddd....wwwwww",
    ],
  });
  asciiTexture("rump_bars", {
    palette: { ".": "#70412f", "w": "#eee7d8", "d": "#332923" },
    pixels: [
      "dd........dd",
      "d..ww..ww..d",
      "..ww..ww....",
      ".ww..ww..ww.",
      "ww..ww..ww..",
      "..ww..ww..ww",
    ],
  });
  asciiTexture("leg_bars", {
    palette: { ".": "#70412f", "w": "#eee7d8", "d": "#332923" },
    pixels: [
      "....",
      "wwww",
      "....",
      "wwww",
      "....",
      "wwww",
      "dddd",
      "dddd",
    ],
  });

  part("body", box({
    at: [0, 1.24, 0.08],
    size: [0.82, 0.68, 1.5],
    material: "coat",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.09, -0.61],
    size: [0.86, 0.7, 0.48],
    material: "coat_dark",
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.01, 0.62],
    size: [0.84, 0.64, 0.54],
    material: "coat",
    faces: {
      east: { texture: "rump_bars" },
      west: { texture: "rump_bars" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.38, 0.02],
    size: [0.66, 0.12, 1.1],
    material: "coat_dark",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.52, -0.65],
    rot: [-25, 0, 0],
    size: [0.46, 0.96, 0.46],
    material: "coat_dark",
    joint: { pivot: [0, -0.46, 0.06], axis: [1, 0, 0] },
  }));
  part("mane", box({
    parent: "neck",
    at: [0, 0.02, 0.27],
    size: [0.13, 0.76, 0.12],
    material: "black",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.58, -0.18],
    rot: [18, 0, 0],
    size: [0.48, 0.5, 0.58],
    material: "coat_dark",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.15, -0.43],
    size: [0.38, 0.27, 0.32],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.29], ["r", 0.29]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.28, 0.08],
      rot: [-4, 0, side === "l" ? -17 : 17],
      size: [0.3, 0.2, 0.12],
      material: "ear_inner",
      joint: { pivot: [side === "l" ? 0.13 : -0.13, 0, 0], axis: [1, 0, 0] },
    }));
    part(`ossicone_${side}`, box({
      parent: "head",
      at: [side === "l" ? -0.12 : 0.12, 0.36, 0.1],
      rot: [-5, 0, 0],
      size: [0.075, 0.2, 0.075],
      material: "ossicone",
    }));
    part(`ossicone_tip_${side}`, box({
      parent: `ossicone_${side}`,
      at: [0, 0.13, 0],
      size: [0.1, 0.09, 0.1],
      material: "black",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.27, -0.51, false],
    ["fr", 0.27, -0.51, false],
    ["bl", -0.28, 0.53, true],
    ["br", 0.28, 0.53, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.68, z],
      size: [0.18, 0.78, 0.19],
      material: rear ? "coat" : "coat_dark",
      faces: {
        north: { texture: "leg_bars" },
        south: { texture: "leg_bars" },
        east: { texture: "leg_bars" },
        west: { texture: "leg_bars" },
      },
      joint: { pivot: [0, 0.39, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.45, -0.03],
      size: [0.22, 0.12, 0.26],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.09, 0.35],
    rot: [-7, 0, 0],
    size: [0.09, 0.55, 0.09],
    material: "coat_dark",
    joint: { pivot: [0, 0.26, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.35, 0.02],
    size: [0.2, 0.2, 0.18],
    material: "black",
  }));

  quadrupedWalk("step", {
    fps: 18,
    duration: 1.02,
    cycleDistance: 0.96,
    gait: "walk",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 2.3,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.64,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.4, lag: 0.12 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.11 }),
    ],
  });
});
