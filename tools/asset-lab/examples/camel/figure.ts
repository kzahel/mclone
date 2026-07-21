import { figure } from "../../src/dsl";

// A box-only adult dromedary. One stepped hump, a long angled neck, narrow
// legs, broad desert feet, and a heavy-lidded face keep the silhouette clear
// without soft primitives or dense surface detail.
export default figure("camel", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#bc874d");
  mat("coat_light", "#d6aa70");
  mat("coat_dark", "#7b5437");
  mat("hump", "#a87243");
  mat("muzzle", "#d9b582");
  mat("ear_inner", "#9d6b58");
  mat("eye", "#17120f");
  mat("foot", "#70513b");

  asciiTexture("face", {
    palette: { ".": "#bc874d", "e": "#17120f", "d": "#7b5437" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#d9b582", "n": "#4b3529" },
    pixels: [
      "........",
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#bc874d", "d": "#7b5437", "l": "#d6aa70" },
    pixels: [
      "..dddd......",
      ".d....d.....",
      "d......d....",
      ".........lll",
      "............",
      "..ll........",
      "............",
    ],
  });

  part("body", box({
    at: [0, 1.52, 0.08],
    size: [0.88, 0.66, 1.55],
    material: "coat",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.05, -0.68],
    size: [0.76, 0.66, 0.34],
    material: "coat_light",
  }));
  part("hump_base", box({
    parent: "body",
    at: [0, 0.42, 0.3],
    size: [0.72, 0.42, 0.68],
    material: "hump",
  }));
  part("hump_peak", box({
    parent: "hump_base",
    at: [0, 0.28, 0.02],
    size: [0.5, 0.28, 0.42],
    material: "hump",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.37, 0.02],
    size: [0.68, 0.12, 1.15],
    material: "coat_dark",
  }));

  part("neck_lower", box({
    parent: "body",
    at: [0, 0.48, -0.72],
    rot: [-28, 0, 0],
    size: [0.42, 0.96, 0.42],
    material: "coat_light",
    joint: { pivot: [0, -0.48, 0.08], axis: [1, 0, 0] },
  }));
  part("neck_upper", box({
    parent: "neck_lower",
    at: [0, 0.6, -0.14],
    rot: [15, 0, 0],
    size: [0.36, 0.62, 0.36],
    material: "coat_light",
  }));
  part("head", box({
    parent: "neck_upper",
    at: [0, 0.4, -0.18],
    rot: [14, 0, 0],
    size: [0.44, 0.4, 0.58],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.43],
    size: [0.46, 0.26, 0.3],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("lower_lip", box({
    parent: "muzzle",
    at: [0, -0.17, -0.03],
    size: [0.34, 0.1, 0.22],
    material: "coat_dark",
  }));
  for (const [side, x] of [["l", -0.2], ["r", 0.2]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.23, 0.08],
      rot: [0, 0, side === "l" ? -15 : 15],
      size: [0.2, 0.2, 0.1],
      material: "ear_inner",
      joint: { pivot: [side === "l" ? 0.07 : -0.07, -0.05, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.29, -0.49],
    ["fr", 0.29, -0.49],
    ["bl", -0.29, 0.5],
    ["br", 0.29, 0.5],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.66, z],
      size: [0.18, 0.7, 0.2],
      material: "coat",
      joint: { pivot: [0, 0.35, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.58, 0.01],
      size: [0.15, 0.5, 0.17],
      material: "coat_light",
    }));
    part(`foot_${suffix}`, box({
      parent: `shin_${suffix}`,
      at: [0, -0.32, -0.07],
      size: [0.3, 0.14, 0.46],
      material: "foot",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, -0.04, 0.86],
    rot: [-7, 0, 0],
    size: [0.08, 0.56, 0.08],
    material: "coat_dark",
    joint: { pivot: [0, 0.28, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.36, 0.02],
    size: [0.17, 0.19, 0.15],
    material: "coat_dark",
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.36,
    cycleDistance: 0.96,
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
    bodyBob: 0.014,
    bodyBobCenter: 0.015,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.68,
    swingDegrees: 16,
    tail: "tail",
    tailSwingDegrees: 7,
    tracks: [
      followThrough("neck_lower", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.12 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.14 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.14 }),
    ],
  });
});
