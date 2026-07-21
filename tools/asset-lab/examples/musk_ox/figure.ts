import { figure } from "../../src/dsl";

// A box-only musk ox with a low woolly barrel, hanging shag curtain, broad
// pale horn boss, short dark legs, and a slow deliberately planted trudge.
export default figure("musk_ox", ({
  asciiTexture,
  box,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
}) => {
  mat("fur", "#50382b");
  mat("fur_light", "#735240");
  mat("fur_dark", "#2e2520");
  mat("shag", "#3b2b24");
  mat("cream", "#c8b995");
  mat("cream_light", "#dfd3b2");
  mat("horn_tip", "#665b49");
  mat("muzzle", "#51483f");
  mat("hoof", "#211d1a");
  mat("eye", "#b78b4c");

  asciiTexture("wool_side", {
    palette: { ".": "#50382b", "l": "#735240", "d": "#2e2520", "s": "#3b2b24" },
    pixels: [
      "llllllllllllll",
      "l............l",
      "..dd..dd..dd..",
      ".d..dd..dd..d.",
      "ssssssssssssss",
      "ss..ss..ss..ss",
      "s..s..s..s..s.",
      "ssssssssssssss",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#50382b", "d": "#2e2520", "e": "#b78b4c", "l": "#735240" },
    pixels: ["dddddddd", "d.e..e.d", "d.e..e.d", "..llll..", ".l....l.", "........"],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#51483f", "n": "#1b1815", "l": "#735240" },
    pixels: ["..llll..", ".llllll.", ".nn..nn.", "..nnnn..", "........"],
  });
  asciiTexture("hoof_face", {
    palette: { ".": "#211d1a", "d": "#0e0c0b" },
    pixels: ["......", "..dd..", "dddddd"],
  });

  part("body", box({
    at: [0, 1.05, 0.08],
    size: [1.24, 0.86, 1.62],
    material: "fur",
    faces: { east: { texture: "wool_side" }, west: { texture: "wool_side" } },
  }));
  part("shoulder", box({
    parent: "body",
    at: [0, 0.18, -0.55],
    size: [1.34, 0.72, 0.68],
    material: "fur_dark",
    faces: { east: { texture: "wool_side" }, west: { texture: "wool_side" } },
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.03, 0.61],
    size: [1.14, 0.76, 0.54],
    material: "fur_light",
  }));
  part("shag_curtain", box({
    parent: "body",
    at: [0, -0.42, 0.06],
    size: [1.1, 0.5, 1.38],
    material: "shag",
    faces: { east: { texture: "wool_side" }, west: { texture: "wool_side" } },
  }));
  part("neck", box({
    parent: "shoulder",
    at: [0, -0.03, -0.53],
    rot: [10, 0, 0],
    size: [0.78, 0.68, 0.52],
    material: "fur_dark",
    joint: { pivot: [0, 0.28, 0.18], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, -0.08, -0.48],
    rot: [3, 0, 0],
    size: [1.0, 0.68, 0.58],
    material: "fur",
    faces: { north: { texture: "face" } },
  }));
  part("forelock", box({
    parent: "head",
    at: [0, 0.37, -0.03],
    size: [0.78, 0.2, 0.44],
    material: "fur_dark",
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.22, -0.41],
    size: [0.62, 0.3, 0.3],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("beard", box({
    parent: "muzzle",
    at: [0, -0.24, 0.02],
    rot: [7, 0, 0],
    size: [0.46, 0.3, 0.22],
    material: "shag",
    joint: { pivot: [0, 0.14, 0], axis: [1, 0, 0] },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.48, 0.13, 0.03],
      rot: [0, 0, sign * 18],
      size: [0.23, 0.14, 0.12],
      material: "fur_light",
      joint: { pivot: [sign * -0.09, 0, 0], axis: [1, 0, 0] },
    }));
    part(`horn_boss_${side}`, box({
      parent: "head",
      at: [sign * 0.24, 0.3, -0.02],
      size: [0.5, 0.22, 0.36],
      material: "cream_light",
    }));
    part(`horn_${side}_1`, box({
      parent: "head",
      at: [sign * 0.5, 0.19, -0.01],
      rot: [0, 0, sign * -110],
      size: [0.18, 0.44, 0.18],
      material: "cream",
      joint: { pivot: [0, -0.19, 0], axis: [0, 0, 1] },
    }));
    part(`horn_${side}_2`, box({
      parent: `horn_${side}_1`,
      at: [0, 0.34, 0],
      rot: [0, 0, sign * 48],
      size: [0.13, 0.34, 0.13],
      material: "cream",
    }));
    part(`horn_${side}_tip`, box({
      parent: `horn_${side}_2`,
      at: [0, 0.27, 0],
      rot: [0, 0, sign * 26],
      size: [0.085, 0.27, 0.085],
      material: "horn_tip",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.42, -0.5, false],
    ["fr", 0.42, -0.5, false],
    ["bl", -0.4, 0.5, true],
    ["br", 0.4, 0.5, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.6, z],
      size: [rear ? 0.25 : 0.29, 0.62, rear ? 0.27 : 0.31],
      material: rear ? "fur_dark" : "shag",
      joint: { pivot: [0, 0.3, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.38, -0.05],
      size: [rear ? 0.3 : 0.34, 0.14, rear ? 0.34 : 0.39],
      material: "hoof",
      faces: { north: { texture: "hoof_face" } },
    }));
  }
  part("tail", box({
    parent: "rump",
    at: [0, 0.07, 0.34],
    rot: [-5, 0, 0],
    size: [0.11, 0.42, 0.11],
    material: "fur_dark",
    joint: { pivot: [0, 0.2, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.28, 0.02],
    size: [0.22, 0.18, 0.2],
    material: "shag",
  }));

  quadrupedWalk("tundra_trudge", {
    label: "Tundra trudge",
    fps: 18,
    duration: 1.5,
    cycleDistance: 0.62,
    gait: "walk",
    loop: true,
    samples: 25,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.009,
    bodyBobCenter: 0.011,
    head: "head",
    headSwingDegrees: 1.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.78,
    swingDegrees: 10,
    tail: "tail",
    tailSwingDegrees: 4,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 2.5, overshoot: 0.32, lag: 0.15 }),
      followThrough("beard", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.6, lag: 0.18 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.42, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.42, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.55, lag: 0.18 }),
    ],
  });
  defaultClip("tundra_trudge");
});
