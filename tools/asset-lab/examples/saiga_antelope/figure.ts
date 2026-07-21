import { figure } from "../../src/dsl";

// A box-only saiga antelope with sandy steppe coat, a low-set trunk-like nose,
// ridged paired horns, cream belly, and a light long-legged trot.
export default figure("saiga_antelope", ({
  asciiTexture,
  box,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
}) => {
  mat("sand", "#b99a69");
  mat("sand_light", "#d5ba86");
  mat("sand_dark", "#856c49");
  mat("cream", "#e4d3ad");
  mat("nose", "#82735f");
  mat("nostril", "#302c27");
  mat("horn", "#b7a16d");
  mat("horn_dark", "#746747");
  mat("hoof", "#3e382f");
  mat("ear_inner", "#9b7063");

  asciiTexture("face", {
    palette: { ".": "#d5ba86", "d": "#856c49", "e": "#211f1c", "l": "#e4d3ad" },
    pixels: ["dd....dd", "d.e..e.d", "..e..e..", "........", ".ll..ll.", "llllllll"],
  });
  asciiTexture("nose_front", {
    palette: { ".": "#82735f", "n": "#302c27", "l": "#a39178" },
    pixels: ["llllllll", "l......l", "........", ".nn..nn.", "nnn..nnn", "........"],
  });
  asciiTexture("coat_band", {
    palette: { ".": "#b99a69", "l": "#d5ba86", "d": "#856c49", "c": "#e4d3ad" },
    pixels: ["llllllllllll", "l..........l", "..dd..dd....", ".d..dd..d...", "............", "cccccccccccc"],
  });
  asciiTexture("horn_ridges", {
    palette: { ".": "#b7a16d", "d": "#746747", "l": "#d0bc86" },
    pixels: ["llll", "dddd", "....", "dddd", "....", "dddd", "....", "dddd"],
  });

  part("body", box({
    at: [0, 1.06, 0.05],
    size: [0.68, 0.62, 1.38],
    material: "sand",
    faces: { east: { texture: "coat_band" }, west: { texture: "coat_band" } },
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.08, -0.64],
    size: [0.58, 0.5, 0.24],
    material: "sand_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.33, 0.04],
    size: [0.54, 0.1, 0.92],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.32, -0.62],
    rot: [-28, 0, 0],
    size: [0.4, 0.68, 0.38],
    material: "sand_dark",
    joint: { pivot: [0, -0.32, 0.08], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.4, -0.2],
    rot: [25, 0, 0],
    size: [0.52, 0.46, 0.52],
    material: "sand_light",
    faces: { north: { texture: "face" } },
  }));
  part("nose_bridge", box({
    parent: "head",
    at: [0, -0.12, -0.38],
    rot: [-8, 0, 0],
    size: [0.4, 0.34, 0.32],
    material: "nose",
  }));
  part("nose_bulb", box({
    parent: "nose_bridge",
    at: [0, -0.16, -0.2],
    rot: [-10, 0, 0],
    size: [0.46, 0.36, 0.28],
    material: "nose",
    faces: { north: { texture: "nose_front" } },
  }));
  part("muzzle", box({
    parent: "nose_bulb",
    at: [0, -0.11, -0.18],
    size: [0.32, 0.18, 0.14],
    material: "nostril",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.29, 0.2, 0.04],
      rot: [-5, 0, sign * 20],
      size: [0.22, 0.17, 0.1],
      material: "ear_inner",
      joint: { pivot: [sign * -0.09, 0, 0], axis: [1, 0, 0] },
    }));
    part(`horn_${side}_1`, box({
      parent: "head",
      at: [sign * 0.14, 0.3, 0.08],
      rot: [15, 0, sign * -3],
      size: [0.09, 0.42, 0.09],
      material: "horn",
      faces: { east: { texture: "horn_ridges" }, west: { texture: "horn_ridges" } },
      joint: { pivot: [0, -0.19, 0], axis: [1, 0, 0] },
    }));
    part(`horn_${side}_2`, box({
      parent: `horn_${side}_1`,
      at: [0, 0.35, -0.05],
      rot: [-12, 0, sign * 2],
      size: [0.07, 0.34, 0.07],
      material: "horn_dark",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.22, -0.48, false],
    ["fr", 0.22, -0.48, false],
    ["bl", -0.23, 0.48, true],
    ["br", 0.23, 0.48, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.56, z],
      size: [0.13, 0.72, 0.14],
      material: rear ? "sand_dark" : "sand",
      joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.44, -0.025],
      size: [0.17, 0.12, 0.22],
      material: "hoof",
    }));
  }
  part("tail", box({
    parent: "body",
    at: [0, 0.1, 0.76],
    rot: [-10, 0, 0],
    size: [0.1, 0.36, 0.1],
    material: "sand_dark",
    joint: { pivot: [0, 0.16, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.24, 0.02],
    size: [0.16, 0.16, 0.15],
    material: "cream",
  }));

  quadrupedWalk("steppe_trot", {
    label: "Steppe trot",
    fps: 20,
    duration: 0.82,
    cycleDistance: 1.1,
    gait: "trot",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.015,
    bodyBobCenter: 0.017,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.56,
    swingDegrees: 23,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4.5, overshoot: 0.48, lag: 0.11 }),
      followThrough("nose_bridge", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2.5, overshoot: 0.35, lag: 0.08 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.1 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.6, lag: 0.16 }),
    ],
  });
  defaultClip("steppe_trot");
});
