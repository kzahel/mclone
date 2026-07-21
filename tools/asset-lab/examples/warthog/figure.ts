import { figure } from "../../src/dsl";

// A box-only warthog with a barrel body, wide low head, cheek bosses, paired
// two-stage tusks, stiff dorsal mane, short legs, and raised tufted tail.
export default figure("warthog", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("hide", "#756a58");
  mat("hide_light", "#97866c");
  mat("hide_dark", "#4b4338");
  mat("mane", "#292621");
  mat("muzzle", "#5b5145");
  mat("tusk", "#e1d2a8");
  mat("tusk_tip", "#b9aa82");
  mat("hoof", "#27231f");
  mat("ear_inner", "#765c54");

  asciiTexture("face", {
    palette: { ".": "#756a58", "d": "#4b4338", "e": "#c7a451", "w": "#97866c" },
    pixels: [
      "dd....dd",
      "d.e..e.d",
      "..e..e..",
      "ww....ww",
      ".ww..ww.",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#5b5145", "n": "#1a1815", "l": "#97866c" },
    pixels: [
      "..llll..",
      ".llllll.",
      ".nn..nn.",
      ".nn..nn.",
      "........",
    ],
  });
  asciiTexture("bristle_side", {
    palette: { ".": "#756a58", "d": "#4b4338", "b": "#292621", "l": "#97866c" },
    pixels: [
      "bbbbbbbbbbbb",
      "bdbdbdbdbdbb",
      "d..........d",
      "..l....l....",
      "............",
      "dddd....dddd",
    ],
  });
  asciiTexture("hoof_face", {
    palette: { "h": "#27231f", "c": "#11100e" },
    pixels: ["hhhhhh", "hhcchh", "cccccc"],
  });

  part("body", box({
    at: [0, 0.9, 0.08],
    size: [0.94, 0.68, 1.28],
    material: "hide",
    faces: {
      east: { texture: "bristle_side" },
      west: { texture: "bristle_side" },
    },
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.12, -0.51],
    size: [1.02, 0.76, 0.52],
    material: "hide_dark",
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.04, 0.51],
    size: [0.9, 0.62, 0.52],
    material: "hide_light",
  }));
  for (const [name, z, height] of [
    ["mane_front", -0.43, 0.28],
    ["mane_mid", 0.02, 0.23],
    ["mane_rear", 0.43, 0.17],
  ] as const) {
    part(name, box({
      parent: "body",
      at: [0, 0.4 + height / 2, z],
      size: [0.16, height, 0.42],
      material: "mane",
    }));
  }
  part("neck", box({
    parent: "shoulders",
    at: [0, -0.02, -0.42],
    rot: [12, 0, 0],
    size: [0.68, 0.58, 0.44],
    material: "hide_dark",
    joint: { pivot: [0, 0, 0.19], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, -0.05, -0.4],
    rot: [5, 0, 0],
    size: [0.72, 0.58, 0.54],
    material: "hide",
    faces: { north: { texture: "face" } },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.14, -0.43],
    size: [0.58, 0.32, 0.34],
    material: "muzzle",
    faces: { north: { texture: "snout_face" } },
  }));
  part("snout_pad", box({
    parent: "snout",
    at: [0, -0.01, -0.2],
    size: [0.44, 0.23, 0.12],
    material: "hide_dark",
    faces: { north: { texture: "snout_face" } },
  }));
  for (const [side, x] of [["l", -0.29], ["r", 0.29]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.34, 0.03],
      rot: [-5, 0, side === "l" ? -18 : 18],
      size: [0.23, 0.27, 0.13],
      material: "ear_inner",
      joint: { pivot: [0, -0.12, 0], axis: [1, 0, 0] },
    }));
    part(`wart_${side}_high`, box({
      parent: "head",
      at: [side === "l" ? -0.39 : 0.39, 0.03, -0.18],
      size: [0.16, 0.15, 0.19],
      material: "hide_light",
    }));
    part(`wart_${side}_low`, box({
      parent: "snout",
      at: [side === "l" ? -0.35 : 0.35, -0.03, -0.03],
      size: [0.14, 0.13, 0.18],
      material: "hide_light",
    }));
    part(`tusk_${side}_1`, box({
      parent: "snout",
      at: [side === "l" ? -0.31 : 0.31, -0.06, -0.27],
      rot: [20, 0, side === "l" ? -28 : 28],
      size: [0.1, 0.36, 0.1],
      material: "tusk",
    }));
    part(`tusk_${side}_2`, box({
      parent: `tusk_${side}_1`,
      at: [0, 0.25, -0.03],
      rot: [-10, 0, side === "l" ? 12 : -12],
      size: [0.075, 0.2, 0.075],
      material: "tusk_tip",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.33, -0.4],
    ["fr", 0.33, -0.4],
    ["bl", -0.32, 0.43],
    ["br", 0.32, 0.43],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.53, z],
      size: [0.2, 0.45, 0.22],
      material: "hide_dark",
      joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.28, -0.05],
      size: [0.26, 0.12, 0.31],
      material: "hoof",
      faces: { north: { texture: "hoof_face" } },
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.18, 0.33],
    rot: [43, 0, 0],
    size: [0.11, 0.48, 0.11],
    material: "hide_dark",
    joint: { pivot: [0, 0.22, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.3, 0.04],
    size: [0.22, 0.2, 0.2],
    material: "mane",
  }));

  quadrupedWalk("trot", {
    fps: 18,
    duration: 0.9,
    cycleDistance: 0.78,
    gait: "trot",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.016,
    head: "neck",
    headSwingDegrees: 3,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.6,
    swingDegrees: 21,
    tail: "tail",
    tailSwingDegrees: 11,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.1 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.15 }),
    ],
  });
});
