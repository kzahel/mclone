import { figure } from "../../src/dsl";

// A box-only butterfly: broad patterned slabs do the visual work instead of
// rounded wing spots, while the thin antennae keep the silhouette readable.
export default figure("butterfly", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
}) => {
  mat("body", "#2b2430");
  mat("body_light", "#5b4a65");
  mat("wing", "#f0a85c");

  asciiTexture("face", {
    palette: {
      ".": "#2b2430",
      "b": "#6ec7d8",
    },
    pixels: [
      "....",
      ".bb.",
      ".bb.",
      "....",
    ],
  });

  asciiTexture("forewing_pattern", {
    palette: {
      ".": "#f0a85c",
      "e": "#2b2430",
      "b": "#6ec7d8",
      "y": "#ffe08a",
    },
    pixels: [
      "eeeeeeee",
      "e..yy..e",
      "e......e",
      "e.b..b.e",
      "e......e",
      "e...y..e",
      "e......e",
      "eeeeeeee",
    ],
  });

  asciiTexture("hindwing_pattern", {
    palette: {
      ".": "#e28a4b",
      "e": "#2b2430",
      "b": "#6ec7d8",
      "y": "#ffe08a",
    },
    pixels: [
      "eeeeeeee",
      "e......e",
      "e.yy...e",
      "e......e",
      "e..bb..e",
      "e......e",
      "e.y....e",
      "eeeeeeee",
    ],
  });

  part("body", box({
    at: [0, 0.34, 0],
    size: [0.14, 0.52, 0.14],
    material: "body",
  }));
  part("thorax", box({
    parent: "body",
    at: [0, 0.04, -0.04],
    size: [0.22, 0.22, 0.2],
    material: "body_light",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.31, -0.02],
    size: [0.2, 0.2, 0.2],
    material: "body",
    faces: {
      north: { texture: "face" },
    },
  }));

  part("antenna_l", box({
    parent: "head",
    at: [-0.08, 0.17, -0.03],
    rot: [0, 0, 24],
    size: [0.025, 0.28, 0.025],
    material: "body",
  }));
  part("antenna_r", box({
    parent: "head",
    at: [0.08, 0.17, -0.03],
    rot: [0, 0, -24],
    size: [0.025, 0.28, 0.025],
    material: "body",
  }));

  part("forewing_l", box({
    parent: "body",
    at: [-0.38, 0.08, -0.07],
    size: [0.62, 0.025, 0.5],
    material: "wing",
    joint: { pivot: [0.31, 0, 0], axis: [0, 0, 1] },
    faces: {
      up: { texture: "forewing_pattern" },
      down: { texture: "forewing_pattern" },
    },
  }));
  part("forewing_r", box({
    parent: "body",
    at: [0.38, 0.08, -0.07],
    size: [0.62, 0.025, 0.5],
    material: "wing",
    joint: { pivot: [-0.31, 0, 0], axis: [0, 0, 1] },
    faces: {
      up: { texture: "forewing_pattern" },
      down: { texture: "forewing_pattern" },
    },
  }));

  part("hindwing_l", box({
    parent: "forewing_l",
    at: [-0.04, 0, 0.38],
    size: [0.48, 0.03, 0.36],
    material: "wing",
    faces: {
      up: { texture: "hindwing_pattern" },
      down: { texture: "hindwing_pattern" },
    },
  }));
  part("hindwing_r", box({
    parent: "forewing_r",
    at: [0.04, 0, 0.38],
    size: [0.48, 0.03, 0.36],
    material: "wing",
    faces: {
      up: { texture: "hindwing_pattern" },
      down: { texture: "hindwing_pattern" },
    },
  }));

  wingFlap("fly", {
    fps: 18,
    duration: 1,
    cycleDistance: 1.15,
    loop: true,
    samples: 13,
    body: "body",
    bodyBob: 0.035,
    degrees: 36,
    frequency: 2,
    leftWing: "forewing_l",
    rightWing: "forewing_r",
  });
});
