import { figure } from "../../src/dsl";

// A classic three-animal chimera: lion forequarters, a goat head rising from
// the spine, and a living serpent that replaces the tail.
export default figure("chimera", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
  quadrupedWalk,
  swing,
}) => {
  metadata({
    bodyPlans: ["quadruped", "serpentine"],
    disposition: "neutral",
    groups: ["animal", "fantasy"],
    habitats: ["land"],
    scale: "large",
    themes: ["chimera", "goat", "hybrid", "lion", "mythic", "serpent"],
  });

  mat("lion", "#b97936");
  mat("lion_light", "#d7a45e");
  mat("lion_dark", "#59341f");
  mat("paw", "#40271a");
  mat("goat", "#d6cfbd");
  mat("goat_shadow", "#9d927d");
  mat("horn", "#846d4e");
  mat("scale", "#597043");
  mat("scale_light", "#89a65d");
  mat("scale_dark", "#2f442d");
  mat("tongue", "#a33f4f");

  asciiTexture("lion_face", {
    palette: { ".": "#b97936", "m": "#59341f", "e": "#e1bd58", "c": "#d7a45e", "n": "#21150f" },
    pixels: [
      "mmmmmmmmmm",
      "m........m",
      "m.ee..ee.m",
      "m.ee..ee.m",
      "..cccccc..",
      ".ccnnnncc.",
      "..ccnncc..",
      "..........",
    ],
  });
  asciiTexture("goat_face", {
    palette: { ".": "#d6cfbd", "e": "#251d17", "s": "#9d927d" },
    pixels: [
      "ss......ss",
      "s.ee..ee.s",
      "..ee..ee..",
      "..........",
      "...ssss...",
      "..ss..ss..",
      "..........",
    ],
  });
  asciiTexture("snake_face", {
    palette: { ".": "#597043", "e": "#ddc354", "p": "#171a10", "d": "#2f442d" },
    pixels: [
      "dddddddd",
      "d.eppe.d",
      "d.eppe.d",
      "d......d",
      "dd....dd",
      "dddddddd",
    ],
  });
  asciiTexture("scale_bands", {
    palette: { ".": "#597043", "l": "#89a65d", "d": "#2f442d" },
    pixels: ["dddddddd", "ll....ll", "........", "..llll..", "........", "dddddddd"],
  });
  asciiTexture("paw_claws", {
    palette: { "p": "#40271a", "c": "#17100c" },
    pixels: ["pppppp", "pcpccp", "pppppp"],
  });

  part("body", box({
    at: [0, 1.04, 0],
    size: [1.02, 0.66, 1.58],
    material: "lion",
  }));
  part("lion_chest", box({
    parent: "body",
    at: [0, 0.04, -0.68],
    size: [1.08, 0.68, 0.46],
    material: "lion_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.35, -0.02],
    size: [0.74, 0.12, 1.08],
    material: "lion_light",
  }));
  part("lion_neck", box({
    parent: "body",
    at: [0, 0.24, -0.84],
    rot: [-12, 0, 0],
    size: [0.72, 0.56, 0.48],
    material: "lion_dark",
    joint: { pivot: [0, -0.25, 0.16], axis: [1, 0, 0] },
  }));
  part("lion_head", box({
    parent: "lion_neck",
    at: [0, 0.13, -0.4],
    rot: [8, 0, 0],
    size: [0.68, 0.52, 0.54],
    material: "lion",
    faces: { north: { texture: "lion_face" } },
  }));
  part("lion_muzzle", box({
    parent: "lion_head",
    at: [0, -0.12, -0.4],
    size: [0.46, 0.24, 0.3],
    material: "lion_light",
  }));
  for (const [side, x] of [["l", -0.27], ["r", 0.27]] as const) {
    part(`lion_ear_${side}`, box({
      parent: "lion_head",
      at: [x, 0.29, 0],
      size: [0.18, 0.18, 0.13],
      material: "lion_dark",
      joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
    }));
  }

  part("goat_neck", box({
    parent: "body",
    at: [0, 0.6, 0.08],
    rot: [-4, 0, 0],
    size: [0.44, 0.7, 0.46],
    material: "goat_shadow",
    joint: { pivot: [0, -0.32, 0], axis: [1, 0, 0] },
  }));
  part("goat_head", box({
    parent: "goat_neck",
    at: [0, 0.45, -0.14],
    rot: [-3, 0, 0],
    size: [0.48, 0.46, 0.54],
    material: "goat",
    faces: { north: { texture: "goat_face" } },
    joint: { pivot: [0, -0.19, 0.18], axis: [0, 1, 0] },
  }));
  part("goat_muzzle", box({
    parent: "goat_head",
    at: [0, -0.14, -0.39],
    size: [0.32, 0.24, 0.28],
    material: "goat_shadow",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`goat_ear_${side}`, box({
      parent: "goat_head",
      at: [sign * 0.31, 0.08, 0.02],
      rot: [0, 0, sign * 34],
      size: [0.28, 0.11, 0.13],
      material: "goat_shadow",
    }));
    part(`horn_${side}`, box({
      parent: "goat_head",
      at: [sign * 0.13, 0.3, 0.1],
      rot: [30, 0, sign * -8],
      size: [0.12, 0.34, 0.12],
      material: "horn",
    }));
    part(`horn_tip_${side}`, box({
      parent: `horn_${side}`,
      at: [0, 0.23, 0.07],
      rot: [28, 0, 0],
      size: [0.075, 0.24, 0.075],
      material: "horn",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.34, -0.47, false],
    ["fr", 0.34, -0.47, false],
    ["bl", -0.36, 0.48, true],
    ["br", 0.36, 0.48, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.55, z],
      size: [rear ? 0.24 : 0.21, rear ? 0.64 : 0.62, rear ? 0.25 : 0.22],
      material: rear ? "lion" : "lion_dark",
      joint: { pivot: [0, rear ? 0.31 : 0.3, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.39 : -0.38, -0.06],
      size: [rear ? 0.31 : 0.29, 0.12, rear ? 0.38 : 0.36],
      material: "paw",
      faces: { north: { texture: "paw_claws" } },
    }));
  }

  part("serpent_1", box({
    parent: "body",
    at: [0, 0.17, 0.97],
    size: [0.38, 0.34, 0.5],
    material: "scale",
    faces: { east: { texture: "scale_bands" }, west: { texture: "scale_bands" } },
    joint: { pivot: [0, 0, -0.22], axis: [0, 1, 0] },
  }));
  part("serpent_2", box({
    parent: "serpent_1",
    at: [0, 0.04, 0.43],
    rot: [-8, 0, 0],
    size: [0.31, 0.29, 0.48],
    material: "scale_light",
    faces: { east: { texture: "scale_bands" }, west: { texture: "scale_bands" } },
    joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] },
  }));
  part("serpent_3", box({
    parent: "serpent_2",
    at: [0, 0.09, 0.4],
    rot: [-14, 0, 0],
    size: [0.25, 0.24, 0.43],
    material: "scale_dark",
    faces: { east: { texture: "scale_bands" }, west: { texture: "scale_bands" } },
    joint: { pivot: [0, 0, -0.19], axis: [0, 1, 0] },
  }));
  part("snake_head", box({
    parent: "serpent_3",
    at: [0, 0.16, 0.38],
    rot: [-20, 0, 0],
    size: [0.38, 0.3, 0.42],
    material: "scale",
    faces: { south: { texture: "snake_face" } },
    joint: { pivot: [0, 0, -0.17], axis: [0, 1, 0] },
  }));
  part("snake_muzzle", box({
    parent: "snake_head",
    at: [0, -0.05, 0.29],
    size: [0.32, 0.19, 0.2],
    material: "scale_light",
  }));
  part("snake_tongue", box({
    parent: "snake_muzzle",
    at: [0, -0.04, 0.2],
    size: [0.05, 0.04, 0.25],
    material: "tongue",
  }));

  quadrupedWalk("threefold_prowl", {
    label: "Threefold prowl",
    fps: 20,
    duration: 1.22,
    cycleDistance: 0.82,
    gait: "walk",
    loop: true,
    samples: 25,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.011,
    bodyBobCenter: 0.012,
    head: "lion_head",
    headSwingDegrees: 2.4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.69,
    swingDegrees: 17,
    tracks: [
      swing("goat_neck", { axis: "z", degrees: 2.3, phase: 0.2 }),
      swing("serpent_1", { axis: "y", degrees: 5, phase: 0.03 }),
      swing("serpent_2", { axis: "y", degrees: 8, phase: 0.14 }),
      swing("serpent_3", { axis: "y", degrees: 11, phase: 0.25 }),
      swing("snake_head", { axis: "y", degrees: 14, phase: 0.36 }),
      followThrough("goat_head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.5, lag: 0.16 }),
    ],
  });
  clip("threefold_threat", {
    label: "Threefold threat",
    role: "action",
    nextClip: "threefold_prowl",
    fps: 30,
    loop: false,
    keys: [
      ["lion_neck", 0, { rot: [0, 0, 0] }],
      ["lion_neck", 0.32, { rot: [-13, 0, 0] }],
      ["lion_neck", 0.78, { rot: [-18, -5, 0] }],
      ["lion_neck", 1.42, { rot: [0, 0, 0] }],
      ["lion_head", 0, { rot: [0, 0, 0] }],
      ["lion_head", 0.32, { rot: [10, 0, 0] }],
      ["lion_head", 0.78, { rot: [14, 7, 0] }],
      ["lion_head", 1.42, { rot: [0, 0, 0] }],
      ["goat_neck", 0, { rot: [0, 0, 0] }],
      ["goat_neck", 0.32, { rot: [-9, 0, -5] }],
      ["goat_neck", 0.78, { rot: [-15, 0, 7] }],
      ["goat_neck", 1.42, { rot: [0, 0, 0] }],
      ["goat_head", 0, { rot: [0, 0, 0] }],
      ["goat_head", 0.32, { rot: [5, -9, 0] }],
      ["goat_head", 0.78, { rot: [8, 11, 0] }],
      ["goat_head", 1.42, { rot: [0, 0, 0] }],
      ["serpent_1", 0, { rot: [0, 0, 0] }],
      ["serpent_1", 0.32, { rot: [0, -12, 0] }],
      ["serpent_1", 0.78, { rot: [0, 16, 0] }],
      ["serpent_1", 1.42, { rot: [0, 0, 0] }],
      ["serpent_2", 0, { rot: [0, 0, 0] }],
      ["serpent_2", 0.32, { rot: [0, -20, 0] }],
      ["serpent_2", 0.78, { rot: [0, 28, 0] }],
      ["serpent_2", 1.42, { rot: [0, 0, 0] }],
      ["serpent_3", 0, { rot: [0, 0, 0] }],
      ["serpent_3", 0.32, { rot: [0, -28, 0] }],
      ["serpent_3", 0.78, { rot: [0, 38, 0] }],
      ["serpent_3", 1.42, { rot: [0, 0, 0] }],
      ["snake_head", 0, { rot: [0, 0, 0] }],
      ["snake_head", 0.32, { rot: [-7, -20, -4] }],
      ["snake_head", 0.78, { rot: [-12, 30, 6] }],
      ["snake_head", 1.42, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("threefold_prowl");
});
