import { figure } from "../../src/dsl";

// A box-only aardvark with an arched back, tall ears, tiny eyes, a long
// tapering snout, powerful digging feet, and a thick three-stage tail.
export default figure("aardvark", ({
  asciiTexture,
  box,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
  swing,
}) => {
  mat("hide", "#927263");
  mat("hide_light", "#b28e7a");
  mat("hide_dark", "#604b45");
  mat("belly", "#c0a28f");
  mat("ear_inner", "#c28583");
  mat("eye", "#1c1a19");
  mat("claw", "#d8c9aa");

  asciiTexture("hide_mottle", {
    palette: { ".": "#927263", "l": "#b28e7a", "d": "#604b45", "b": "#c0a28f" },
    pixels: [
      "llllllllllll",
      "l..d.....d.l",
      "l.....d....l",
      "l.d......d.l",
      "..bbbbbbbb..",
      "bbbbbbbbbbbb",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#b28e7a", "e": "#1c1a19", "d": "#604b45" },
    pixels: [
      "dd......dd",
      ".ee....ee.",
      "..e....e..",
      "..........",
      "...dddd...",
      "..........",
    ],
  });
  asciiTexture("nose", {
    palette: { ".": "#604b45", "n": "#1c1a19" },
    pixels: ["........", "..nnnn..", ".nnnnnn.", "..nnnn.."],
  });
  asciiTexture("claws", {
    palette: { ".": "#604b45", "c": "#d8c9aa" },
    pixels: ["........", ".c.c.c..", "cccccccc"],
  });

  part("body", box({
    at: [0, 0.72, 0.08],
    size: [0.78, 0.52, 1.2],
    material: "hide",
    faces: {
      east: { texture: "hide_mottle" },
      west: { texture: "hide_mottle" },
    },
  }));
  part("back", box({
    parent: "body",
    at: [0, 0.29, 0.12],
    size: [0.68, 0.24, 0.76],
    material: "hide_light",
    faces: { up: { texture: "hide_mottle" } },
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.03, 0.46],
    size: [0.82, 0.55, 0.42],
    material: "hide",
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.02, -0.46],
    size: [0.72, 0.54, 0.4],
    material: "hide_dark",
  }));
  part("neck", box({
    parent: "chest",
    at: [0, 0.08, -0.34],
    rot: [8, 0, 0],
    size: [0.42, 0.42, 0.36],
    material: "hide_dark",
    joint: { pivot: [0, 0, 0.15], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.06, -0.33],
    rot: [3, 0, 0],
    size: [0.54, 0.42, 0.42],
    material: "hide_light",
    faces: { north: { texture: "face" } },
  }));
  part("snout_1", box({
    parent: "head",
    at: [0, -0.1, -0.38],
    size: [0.34, 0.24, 0.5],
    material: "hide_light",
  }));
  part("snout_2", box({
    parent: "snout_1",
    at: [0, -0.04, -0.35],
    size: [0.25, 0.19, 0.32],
    material: "hide_dark",
  }));
  part("nose", box({
    parent: "snout_2",
    at: [0, 0, -0.21],
    size: [0.2, 0.16, 0.13],
    material: "hide_dark",
    faces: { north: { texture: "nose" } },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.18, 0.38, 0.08],
      rot: [-5, 0, sign * -8],
      size: [0.19, 0.46, 0.15],
      material: "hide_dark",
      joint: { pivot: [0, -0.21, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0.02, -0.09],
      size: [0.1, 0.31, 0.04],
      material: "ear_inner",
    }));
  }

  for (const [suffix, x, z, front] of [
    ["fl", -0.27, -0.36, true],
    ["fr", 0.27, -0.36, true],
    ["bl", -0.28, 0.38, false],
    ["br", 0.28, 0.38, false],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.42, z],
      size: [front ? 0.19 : 0.17, 0.38, front ? 0.2 : 0.18],
      material: front ? "hide_dark" : "hide",
      joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.24, -0.07],
      size: [front ? 0.33 : 0.28, 0.1, front ? 0.36 : 0.32],
      material: "hide_dark",
      faces: { north: { texture: "claws" } },
    }));
  }

  part("tail_1", box({
    parent: "rump",
    at: [0, -0.02, 0.39],
    rot: [-8, 0, 0],
    size: [0.5, 0.4, 0.62],
    material: "hide",
    joint: { pivot: [0, 0, -0.28], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.03, 0.5],
    rot: [-7, 0, 0],
    size: [0.35, 0.29, 0.52],
    material: "hide_light",
    joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, -0.02, 0.4],
    rot: [-5, 0, 0],
    size: [0.2, 0.18, 0.36],
    material: "hide_dark",
  }));

  quadrupedWalk("forage", {
    label: "Foraging walk",
    role: "locomotion",
    fps: 18,
    duration: 1.18,
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
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.7,
    swingDegrees: 16,
    tail: "tail_1",
    tailSwingDegrees: 5,
    tracks: [
      swing("tail_2", { axis: "y", degrees: 7, phase: 0.14 }),
      swing("tail_tip", { axis: "y", degrees: 9, phase: 0.24 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.11 }),
    ],
  });
  defaultClip("forage");
});
