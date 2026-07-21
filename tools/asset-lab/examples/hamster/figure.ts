import { figure } from "../../src/dsl";

// A box-only golden hamster with a compact rounded silhouette, stuffed cheek
// pouches, tiny paws, dark bead eyes, and a nearly hidden tail.
export default figure("hamster", ({
  asciiTexture,
  box,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
}) => {
  mat("gold", "#b8793f");
  mat("gold_light", "#d29c61");
  mat("cream", "#ead8b8");
  mat("brown", "#765034");
  mat("pink", "#d99393");
  mat("black", "#211d19");

  asciiTexture("fur", {
    palette: { ".": "#b8793f", "l": "#d29c61", "c": "#ead8b8" },
    pixels: [
      "llllllllll",
      "l........l",
      "l..ll....l",
      "l......l.l",
      "l.cccccc.l",
      "cccccccccc",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#d29c61", "c": "#ead8b8", "e": "#211d19", "n": "#d99393" },
    pixels: [
      "..........",
      ".ee....ee.",
      ".ee....ee.",
      "..cc..cc..",
      ".cccccccc.",
      "....nn....",
    ],
  });
  asciiTexture("toes", {
    palette: { ".": "#d99393", "d": "#765034" },
    pixels: [".d.d.", "ddddd", "....."],
  });

  part("body", box({
    at: [0, 0.41, 0.08],
    size: [0.72, 0.56, 0.76],
    material: "gold",
    faces: {
      east: { texture: "fur" },
      west: { texture: "fur" },
    },
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.03, 0.28],
    size: [0.76, 0.58, 0.42],
    material: "gold_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.31, -0.04],
    size: [0.52, 0.12, 0.5],
    material: "cream",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.02, -0.45],
    size: [0.62, 0.5, 0.48],
    material: "gold_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.2], axis: [0, 1, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.31],
    size: [0.34, 0.22, 0.18],
    material: "cream",
  }));
  part("nose", box({
    parent: "muzzle",
    at: [0, 0.02, -0.13],
    size: [0.12, 0.1, 0.08],
    material: "pink",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`cheek_${side}`, box({
      parent: "head",
      at: [sign * 0.29, -0.11, -0.12],
      size: [0.22, 0.3, 0.3],
      material: "cream",
    }));
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.2, 0.3, 0.03],
      rot: [0, 0, sign * -9],
      size: [0.2, 0.2, 0.1],
      material: "pink",
      joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
    }));
  }
  for (const [suffix, x, z, rear] of [
    ["fl", -0.22, -0.2, false],
    ["fr", 0.22, -0.2, false],
    ["bl", -0.24, 0.25, true],
    ["br", 0.24, 0.25, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.29, z],
      size: [rear ? 0.17 : 0.14, 0.2, rear ? 0.17 : 0.14],
      material: rear ? "brown" : "gold",
      joint: { pivot: [0, 0.09, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.13, -0.055],
      size: [0.2, 0.07, 0.22],
      material: "pink",
      faces: { up: { texture: "toes" } },
    }));
  }
  part("tail", box({
    parent: "rump",
    at: [0, -0.04, 0.28],
    rot: [-18, 0, 0],
    size: [0.12, 0.11, 0.18],
    material: "pink",
  }));

  quadrupedWalk("scurry", {
    label: "Cheeky scurry",
    role: "locomotion",
    fps: 24,
    duration: 0.46,
    cycleDistance: 0.38,
    gait: "trot",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.56,
    swingDegrees: 24,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.1 }),
    ],
  });
  defaultClip("scurry");
});
