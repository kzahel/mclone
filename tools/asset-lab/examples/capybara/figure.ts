import { figure } from "../../src/dsl";

// A box-only adult capybara with a long russet barrel, high blunt head, tiny
// rounded-by-stepping ears, short planted legs, and no visible external tail.
// Its slow walk emphasizes the animal's calm, heavy water-edge posture.
export default figure("capybara", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#8a5a35");
  mat("fur_light", "#a87345");
  mat("fur_dark", "#563b29");
  mat("belly", "#b38a60");
  mat("muzzle", "#a97952");
  mat("nose", "#2a211d");
  mat("eye", "#171411");
  mat("paw", "#493426");

  asciiTexture("face", {
    palette: { ".": "#a87345", "d": "#563b29", "e": "#171411", "l": "#c08d5b" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      ".ee..ee.",
      "..llll..",
      ".llllll.",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#a97952", "n": "#2a211d", "l": "#c29669" },
    pixels: [
      "lll..lll",
      ".lnnnnl.",
      "..nnnn..",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#8a5a35", "l": "#a87345", "d": "#563b29", "b": "#b38a60" },
    pixels: [
      "dddddddddddddd",
      "dlllllllllllld",
      "l............l",
      "..............",
      "..bbbbbbbbbb..",
      ".bbbbbbbbbbbb.",
    ],
  });

  part("body", box({
    at: [0, 0.84, 0.08],
    size: [1.0, 0.68, 1.46],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.39, 0],
    size: [0.78, 0.12, 1.08],
    material: "belly",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.03, 0.58],
    size: [1.02, 0.61, 0.5],
    material: "fur_light",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.08, -0.58],
    size: [1.02, 0.66, 0.46],
    material: "fur_light",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.1, -0.91],
    size: [0.7, 0.66, 0.62],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.27], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.16, -0.45],
    size: [0.72, 0.38, 0.38],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.26], ["r", 0.26]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.39, 0.08],
      size: [0.16, 0.17, 0.12],
      material: "fur_dark",
      joint: { pivot: [0, -0.065, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.34, -0.42],
    ["fr", 0.34, -0.42],
    ["bl", -0.34, 0.46],
    ["br", 0.34, 0.46],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.52, z],
      size: [0.19, 0.46, 0.21],
      material: "fur_dark",
      joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.28, -0.055],
      size: [0.27, 0.1, 0.31],
      material: "paw",
    }));
  }

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.38,
    cycleDistance: 0.66,
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
    bodyBob: 0.016,
    bodyBobCenter: 0.018,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.74,
    swingDegrees: 13,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.4, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.4, lag: 0.12 }),
    ],
  });
});
