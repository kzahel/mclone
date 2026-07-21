import { figure } from "../../src/dsl";

// A box-only striped skunk with a broad white dorsal blaze, small pointed
// face, short planted legs, an oversized curled plume, and a warning handstand.
export default figure("skunk", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
  swing,
}) => {
  mat("black", "#242526");
  mat("black_light", "#3b3c3c");
  mat("white", "#e6e1d5");
  mat("cream", "#cfc7b8");
  mat("pink", "#b97d7e");
  mat("paw", "#171819");

  asciiTexture("back_stripes", {
    palette: { ".": "#242526", "w": "#e6e1d5", "g": "#3b3c3c" },
    pixels: [
      "gggwwwwwwggg",
      "ggww....wwgg",
      ".ww......ww.",
      "ww........ww",
      "w..........w",
      "............",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#242526", "w": "#e6e1d5", "e": "#171819", "p": "#b97d7e" },
    pixels: [
      "ww......ww",
      "wee....eew",
      ".we....ew.",
      "..wwwwww..",
      "...wppw...",
      "..........",
    ],
  });
  asciiTexture("tail_stripe", {
    palette: { ".": "#242526", "w": "#e6e1d5", "g": "#3b3c3c" },
    pixels: [
      "gggwwwwwggg",
      "ggww...wwgg",
      "gww.....wwg",
      "ww.......ww",
      "w.........w",
      "ggggggggggg",
    ],
  });

  part("body", box({
    at: [0, 0.64, 0.06],
    size: [0.74, 0.5, 1.05],
    material: "black",
    faces: {
      up: { texture: "back_stripes" },
      east: { texture: "back_stripes" },
      west: { texture: "back_stripes" },
    },
    joint: { pivot: [0, -0.64, -0.34], axis: [1, 0, 0] },
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.04, 0.4],
    size: [0.78, 0.52, 0.46],
    material: "black_light",
    faces: { up: { texture: "back_stripes" } },
  }));
  part("chest", box({
    parent: "body",
    at: [0, 0.02, -0.42],
    size: [0.7, 0.48, 0.42],
    material: "black_light",
  }));
  part("neck", box({
    parent: "chest",
    at: [0, 0.08, -0.34],
    size: [0.44, 0.36, 0.42],
    material: "white",
    joint: { pivot: [0, 0, 0.19], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.02, -0.31],
    size: [0.56, 0.43, 0.43],
    material: "black",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.19], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.31],
    size: [0.32, 0.18, 0.2],
    material: "cream",
  }));
  part("nose", box({
    parent: "muzzle",
    at: [0, 0.01, -0.14],
    size: [0.16, 0.11, 0.08],
    material: "pink",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.19, 0.27, 0.03],
      rot: [0, 0, sign * -8],
      size: [0.16, 0.23, 0.11],
      material: "black_light",
      joint: { pivot: [0, -0.1, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.25, -0.32, false],
    ["fr", 0.25, -0.32, false],
    ["bl", -0.26, 0.34, true],
    ["br", 0.26, 0.34, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.37, z],
      size: [rear ? 0.17 : 0.15, 0.34, 0.17],
      material: "black_light",
      joint: { pivot: [0, 0.16, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.21, -0.06],
      size: [0.24, 0.1, 0.27],
      material: "paw",
    }));
  }

  part("tail_1", box({
    parent: "rump",
    at: [0, 0.16, 0.4],
    rot: [-34, 0, 0],
    size: [0.5, 0.44, 0.56],
    material: "black",
    faces: { east: { texture: "tail_stripe" }, west: { texture: "tail_stripe" } },
    joint: { pivot: [0, 0, -0.25], axis: [1, 0, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, 0.02, 0.46],
    rot: [-30, 0, 0],
    size: [0.58, 0.5, 0.5],
    material: "white",
    faces: { east: { texture: "tail_stripe" }, west: { texture: "tail_stripe" } },
    joint: { pivot: [0, 0, -0.22], axis: [1, 0, 0] },
  }));
  part("tail_3", box({
    parent: "tail_2",
    at: [0, 0.02, 0.39],
    rot: [-26, 0, 0],
    size: [0.52, 0.44, 0.4],
    material: "black_light",
    faces: { east: { texture: "tail_stripe" }, west: { texture: "tail_stripe" } },
    joint: { pivot: [0, 0, -0.18], axis: [1, 0, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_3",
    at: [0, 0.02, 0.3],
    rot: [-18, 0, 0],
    size: [0.4, 0.34, 0.3],
    material: "white",
    faces: { east: { texture: "tail_stripe" }, west: { texture: "tail_stripe" } },
  }));

  quadrupedWalk("amble", {
    label: "Cautious amble",
    role: "locomotion",
    fps: 18,
    duration: 1.08,
    cycleDistance: 0.58,
    gait: "walk",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.012,
    head: "neck",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.69,
    swingDegrees: 17,
    tail: "tail_1",
    tailSwingDegrees: 4,
    tracks: [
      swing("tail_2", { axis: "y", degrees: 5, phase: 0.12 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.11 }),
    ],
  });
  clip("warning_handstand", {
    label: "Warning handstand",
    role: "action",
    nextClip: "amble",
    fps: 24,
    loop: false,
    keys: [
      ["body", 0, { rot: [0, 0, 0] }],
      ["body", 0.18, { rot: [0, 0, 0] }],
      ["body", 0.42, { rot: [-45, 0, 0] }],
      ["body", 0.9, { rot: [-45, 0, 0] }],
      ["body", 1.18, { rot: [0, 0, 0] }],
      ["body", 1.4, { rot: [0, 0, 0] }],
      ["chest", 0, { at: [0, 0, 0] }],
      ["chest", 0.42, { at: [0, 0.08, 0] }],
      ["chest", 0.9, { at: [0, 0.08, 0] }],
      ["chest", 1.18, { at: [0, 0, 0] }],
      ["chest", 1.4, { at: [0, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.42, { rot: [65, 0, 0] }],
      ["head", 0.9, { rot: [65, 0, 0] }],
      ["head", 1.18, { rot: [0, 0, 0] }],
      ["head", 1.4, { rot: [0, 0, 0] }],
      ["leg_fl", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["leg_fl", 0.42, { at: [0, 0, 0], rot: [46, 0, 0] }],
      ["leg_fl", 0.9, { at: [0, 0, 0], rot: [46, 0, 0] }],
      ["leg_fl", 1.18, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["leg_fr", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["leg_fr", 0.42, { at: [0, 0, 0], rot: [46, 0, 0] }],
      ["leg_fr", 0.9, { at: [0, 0, 0], rot: [46, 0, 0] }],
      ["leg_fr", 1.18, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["tail_1", 0, { rot: [0, 0, 0] }],
      ["tail_1", 0.42, { rot: [12, 0, 0] }],
      ["tail_1", 0.9, { rot: [12, 0, 0] }],
      ["tail_1", 1.18, { rot: [0, 0, 0] }],
      ["tail_1", 1.4, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("amble");
});
