import { figure } from "../../src/dsl";

// A box-only Atlantic walrus with a massive wrinkled body, whisker pads,
// paired tusks, broad foreflippers, and split hind flippers.
export default figure("walrus", ({
  asciiTexture,
  box,
  defaultClip,
  mat,
  part,
  swim,
  swing,
}) => {
  mat("hide", "#8f6856");
  mat("hide_light", "#a87e69");
  mat("hide_dark", "#684d43");
  mat("belly", "#b08c78");
  mat("muzzle", "#b59b83");
  mat("tusk", "#eee2bd");
  mat("nose", "#4b3834");

  asciiTexture("wrinkles", {
    palette: { ".": "#8f6856", "l": "#a87e69", "d": "#684d43", "b": "#b08c78" },
    pixels: [
      "llllllllllllll",
      "l..d......d..l",
      "l....d..d....l",
      "l.d........d.l",
      "l...d....d...l",
      "bbbbbbbbbbbbbb",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#a87e69", "e": "#302722", "m": "#b59b83", "n": "#4b3834" },
    pixels: [
      "............",
      "..ee....ee..",
      "..ee....ee..",
      ".mmmmmmmmmm.",
      "mmmmnnnnmmmm",
      "mmmmmmmmmmmm",
    ],
  });
  asciiTexture("whisker_pad", {
    palette: { ".": "#b59b83", "w": "#eadcc2", "d": "#684d43" },
    pixels: [
      "w..w..w.",
      ".w..w..w",
      "..d..d..",
      "w..w..w.",
      ".w..w..w",
    ],
  });

  part("body", box({
    at: [0, 0.62, 0.12],
    size: [1.16, 0.88, 1.68],
    material: "hide",
    faces: {
      east: { texture: "wrinkles" },
      west: { texture: "wrinkles" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.48, -0.04],
    size: [0.92, 0.18, 1.28],
    material: "belly",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.05, -0.62],
    size: [1.24, 0.8, 0.62],
    material: "hide_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.06, 0.66],
    size: [0.96, 0.7, 0.54],
    material: "hide_dark",
  }));
  part("head", box({
    parent: "shoulders",
    at: [0, 0.04, -0.48],
    size: [1.12, 0.72, 0.54],
    material: "hide_light",
    faces: { north: { texture: "face" } },
  }));
  part("nose", box({
    parent: "head",
    at: [0, -0.05, -0.36],
    size: [0.32, 0.2, 0.2],
    material: "nose",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`muzzle_${side}`, box({
      parent: "head",
      at: [sign * 0.25, -0.15, -0.36],
      size: [0.46, 0.36, 0.28],
      material: "muzzle",
      faces: { north: { texture: "whisker_pad" } },
    }));
    part(`tusk_${side}`, box({
      parent: `muzzle_${side}`,
      at: [sign * 0.11, -0.29, -0.06],
      rot: [-7, 0, sign * -4],
      size: [0.13, 0.44, 0.13],
      material: "tusk",
    }));
    part(`foreflipper_${side}`, box({
      parent: "body",
      at: [sign * 0.7, -0.32, -0.38],
      rot: [2, sign * -12, sign * -9],
      size: [0.68, 0.12, 0.46],
      material: "hide_dark",
      joint: { pivot: [sign * -0.31, 0, -0.08], axis: [0, 0, 1] },
    }));
  }
  part("pelvis", box({
    parent: "rump",
    at: [0, -0.03, 0.43],
    size: [0.54, 0.38, 0.48],
    material: "hide_dark",
    joint: { pivot: [0, 0, -0.21], axis: [1, 0, 0] },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`hind_flipper_${side}`, box({
      parent: "pelvis",
      at: [sign * 0.3, -0.01, 0.37],
      rot: [0, sign * -14, sign * 5],
      size: [0.54, 0.11, 0.55],
      material: "hide_dark",
      joint: { pivot: [sign * -0.22, 0, -0.22], axis: [0, 0, 1] },
    }));
  }

  swim("swim", {
    label: "Heavy swim",
    role: "locomotion",
    fps: 18,
    duration: 1.22,
    cycleDistance: 1.18,
    loop: true,
    samples: 21,
    body: "body",
    bodyBob: 0.022,
    bodySwayDegrees: 1.5,
    finAxis: "z",
    finPhase: 0.16,
    finSwingDegrees: 13,
    leftFin: "foreflipper_l",
    rightFin: "foreflipper_r",
    tail: "pelvis",
    tailAxis: "x",
    tailSwingDegrees: 10,
    tracks: [
      swing("hind_flipper_l", { axis: "z", degrees: 8, phase: 0.12 }),
      swing("hind_flipper_r", { axis: "z", degrees: -8, phase: 0.12 }),
      swing("tusk_l", { axis: "x", degrees: 1.2, phase: 0.18 }),
      swing("tusk_r", { axis: "x", degrees: 1.2, phase: 0.18 }),
    ],
  });
  defaultClip("swim");
});
