import { figure } from "../../src/dsl";

// A box-only common raven with a heavy head and beak, shaggy throat, long
// blue-black body, broad layered wings, and a three-plane wedge tail.
export default figure("raven", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
  followThrough,
}) => {
  mat("black", "#171a1c");
  mat("blue_black", "#252d35");
  mat("gloss", "#3c4b59");
  mat("shadow", "#0d0f10");
  mat("eye", "#878b82");
  mat("foot", "#272727");

  asciiTexture("face", {
    palette: { ".": "#252d35", "b": "#171a1c", "g": "#3c4b59", "e": "#878b82", "s": "#0d0f10" },
    pixels: [
      "ggbbbbgg",
      "g.e..e.g",
      "..e..e..",
      "bbssssbb",
      "bssssssb",
      "bbbbbbbb",
    ],
  });
  asciiTexture("wing_coverts", {
    palette: { ".": "#252d35", "g": "#3c4b59", "b": "#171a1c", "s": "#0d0f10" },
    pixels: [
      "gggggggggggg",
      "g..........g",
      "..gg..gg..gg",
      ".g..gg..gg..",
      "bbb....bbbbb",
      "b..bbbb....b",
      "ssssssssssss",
    ],
  });
  asciiTexture("flight_feathers", {
    palette: { ".": "#171a1c", "g": "#3c4b59", "s": "#0d0f10" },
    pixels: [
      "gggggggggg",
      "g........g",
      "ss..ss..ss",
      "s.ss..ss.s",
      "ss..ss..ss",
      "ssssssssss",
    ],
  });
  asciiTexture("tail_gloss", {
    palette: { ".": "#171a1c", "g": "#3c4b59", "s": "#0d0f10" },
    pixels: ["gggggggg", "g......g", "..gg....", "........", "ssssssss"],
  });

  part("body", box({
    at: [0, 0.9, 0.08],
    size: [0.68, 0.7, 1.02],
    material: "blue_black",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.03, -0.57],
    size: [0.54, 0.56, 0.18],
    material: "black",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.25, -0.5],
    size: [0.5, 0.52, 0.38],
    material: "black",
    joint: { pivot: [0, -0.2, 0.12], axis: [1, 0, 0] },
  }));
  part("throat_shag", box({
    parent: "neck",
    at: [0, -0.18, -0.23],
    size: [0.34, 0.28, 0.12],
    material: "shadow",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.2, -0.32],
    size: [0.54, 0.5, 0.5],
    material: "blue_black",
    faces: { north: { texture: "face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.04, -0.43],
    rot: [5, 0, 0],
    size: [0.34, 0.25, 0.46],
    material: "black",
  }));
  part("beak_tip", box({
    parent: "beak",
    at: [0, -0.04, -0.29],
    rot: [10, 0, 0],
    size: [0.24, 0.18, 0.18],
    material: "shadow",
  }));

  for (const [side, x] of [["l", -0.65], ["r", 0.65]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [x, 0.12, 0.02],
      size: [1.08, 0.09, 0.84],
      material: "blue_black",
      faces: {
        up: { texture: "wing_coverts" },
        down: { texture: "wing_coverts" },
      },
      joint: { pivot: [side === "l" ? 0.53 : -0.53, 0, -0.12], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [side === "l" ? -0.78 : 0.78, -0.01, 0.08],
      rot: [0, side === "l" ? 7 : -7, 0],
      size: [0.58, 0.07, 0.68],
      material: "black",
      faces: {
        up: { texture: "flight_feathers" },
        down: { texture: "flight_feathers" },
      },
    }));
  }
  for (const [name, x, yaw] of [
    ["tail_l", -0.2, 6],
    ["tail_c", 0, 0],
    ["tail_r", 0.2, -6],
  ] as const) {
    part(name, box({
      parent: "body",
      at: [x, 0.02, 0.79],
      rot: [7, yaw, 0],
      size: [0.24, 0.08, name === "tail_c" ? 0.78 : 0.68],
      material: name === "tail_c" ? "shadow" : "black",
      faces: {
        up: { texture: "tail_gloss" },
        down: { texture: "tail_gloss" },
      },
      joint: { pivot: [0, 0, -0.34], axis: [1, 0, 0] },
    }));
  }
  for (const [side, x] of [["l", -0.19], ["r", 0.19]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.42, -0.04],
      rot: [-38, 0, 0],
      size: [0.12, 0.28, 0.13],
      material: "foot",
    }));
    part(`talon_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.14, 0.09],
      size: [0.23, 0.08, 0.3],
      material: "shadow",
    }));
  }

  wingFlap("soar", {
    fps: 18,
    duration: 1.16,
    cycleDistance: 1.55,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.042,
    degrees: 31,
    leftWing: "wing_l",
    rightWing: "wing_r",
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.4, lag: 0.12 }),
      followThrough("tail_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.17 }),
      followThrough("tail_c", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.18 }),
      followThrough("tail_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.17 }),
    ],
  });
});
