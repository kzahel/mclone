import { figure } from "../../src/dsl";

// A box-only Eurasian lynx with a high rump, long legs, snowshoe paws,
// cheek ruffs, black ear tufts, a spotted coat, and a short dark-tipped tail.
export default figure("lynx", ({
  asciiTexture,
  box,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
  swing,
}) => {
  mat("coat", "#a98258");
  mat("coat_light", "#c6a77b");
  mat("coat_dark", "#5b4938");
  mat("ruff", "#d7c39d");
  mat("black", "#292a28");
  mat("paw", "#b89d78");

  asciiTexture("spotted_coat", {
    palette: { ".": "#a98258", "l": "#c6a77b", "d": "#5b4938" },
    pixels: [
      "llllllllllll",
      "l..d....d..l",
      "l.....d....l",
      "l.d......d.l",
      "l....d.....l",
      "llllllllllll",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#a98258", "r": "#d7c39d", "e": "#b7c05a", "b": "#292a28" },
    pixels: [
      "r........r",
      "r.ee..ee.r",
      "..eb..be..",
      "...bbbb...",
      "..rrrrrr..",
      ".rr....rr.",
    ],
  });
  asciiTexture("paw_spots", {
    palette: { ".": "#b89d78", "d": "#5b4938" },
    pixels: [".d.d.d.", "ddddddd", "......."],
  });

  part("body", box({
    at: [0, 0.77, 0.05],
    size: [0.82, 0.5, 1.08],
    material: "coat",
    faces: {
      east: { texture: "spotted_coat" },
      west: { texture: "spotted_coat" },
    },
  }));
  part("chest", box({
    parent: "body",
    at: [0, 0.07, -0.43],
    size: [0.88, 0.62, 0.5],
    material: "coat_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.08, 0.42],
    size: [0.86, 0.58, 0.48],
    material: "coat",
    faces: {
      east: { texture: "spotted_coat" },
      west: { texture: "spotted_coat" },
    },
  }));
  part("neck", box({
    parent: "chest",
    at: [0, 0.08, -0.38],
    rot: [8, 0, 0],
    size: [0.56, 0.46, 0.42],
    material: "ruff",
    joint: { pivot: [0, 0, 0.18], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.06, -0.35],
    rot: [2, 0, 0],
    size: [0.68, 0.54, 0.52],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.34],
    size: [0.42, 0.2, 0.2],
    material: "ruff",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`cheek_${side}`, box({
      parent: "head",
      at: [sign * 0.35, -0.1, -0.03],
      rot: [0, 0, sign * 12],
      size: [0.18, 0.34, 0.34],
      material: "ruff",
    }));
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.22, 0.35, 0],
      rot: [0, 0, sign * -8],
      size: [0.17, 0.28, 0.13],
      material: "coat_dark",
      joint: { pivot: [0, -0.13, 0], axis: [1, 0, 0] },
    }));
    part(`ear_tuft_${side}`, box({
      parent: `ear_${side}`,
      at: [sign * 0.025, 0.22, 0],
      rot: [0, 0, sign * -5],
      size: [0.045, 0.22, 0.045],
      material: "black",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.3, -0.34, false],
    ["fr", 0.3, -0.34, false],
    ["bl", -0.3, 0.36, true],
    ["br", 0.3, 0.36, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.41, z],
      size: [0.17, rear ? 0.5 : 0.47, 0.18],
      material: rear ? "coat_dark" : "coat",
      joint: { pivot: [0, rear ? 0.24 : 0.225, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.3 : -0.285, -0.07],
      size: [0.28, 0.1, 0.3],
      material: "paw",
      faces: { up: { texture: "paw_spots" } },
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.08, 0.4],
    rot: [-18, 0, 0],
    size: [0.2, 0.18, 0.48],
    material: "coat",
    joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.02, 0.36],
    rot: [-8, 0, 0],
    size: [0.18, 0.17, 0.28],
    material: "black",
  }));

  quadrupedWalk("prowl", {
    label: "Snowshoe prowl",
    role: "locomotion",
    fps: 18,
    duration: 0.94,
    cycleDistance: 0.82,
    gait: "walk",
    loop: true,
    samples: 19,
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
    headSwingDegrees: 2.4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.66,
    swingDegrees: 20,
    tail: "tail",
    tailSwingDegrees: 7,
    tracks: [
      swing("tail_tip", { axis: "y", degrees: 9, phase: 0.16 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.5, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.5, lag: 0.12 }),
    ],
  });
  defaultClip("prowl");
});
