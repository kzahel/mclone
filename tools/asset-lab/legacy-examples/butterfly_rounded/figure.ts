import { legacyFigure } from "../../src/dsl";

export default legacyFigure("butterfly_rounded", ({ mat, asciiTexture, part, box, capsule, sphere, cylinder, wingFlap }) => {
  mat("body", "#2b2430");
  mat("body_light", "#5b4a65");
  mat("wing", "#f0a85c");
  mat("wing_edge", "#2b2430");
  mat("spot_blue", "#6ec7d8");
  mat("spot_yellow", "#ffe08a");

  asciiTexture("wing_pattern", {
    palette: {
      ".": "#f0a85c",
      "e": "#2b2430",
      "b": "#6ec7d8",
      "y": "#ffe08a",
    },
    pixels: [
      "eeeeeeee",
      "e..yy..e",
      "e.b..b.e",
      "e......e",
      "e..bb..e",
      "e.y..y.e",
      "e......e",
      "eeeeeeee",
    ],
  });

  part("body", capsule({ at: [0, 0.34, 0], radius: 0.07, length: 0.52, material: "body" }));
  part("thorax", sphere({ parent: "body", at: [0, 0.04, -0.04], radius: 0.11, material: "body_light" }));
  part("head", sphere({ parent: "body", at: [0, 0.31, -0.02], radius: 0.1, material: "body" }));
  part("eye_l", sphere({ parent: "head", at: [-0.045, 0.015, -0.08], radius: 0.025, material: "spot_blue" }));
  part("eye_r", sphere({ parent: "head", at: [0.045, 0.015, -0.08], radius: 0.025, material: "spot_blue" }));

  part("antenna_l", cylinder({ parent: "head", at: [-0.09, 0.11, -0.03], rot: [0, 0, -24], radius: 0.01, length: 0.24, radialSegments: 6, material: "body" }));
  part("antenna_r", cylinder({ parent: "head", at: [0.09, 0.11, -0.03], rot: [0, 0, 24], radius: 0.01, length: 0.24, radialSegments: 6, material: "body" }));
  part("antenna_tip_l", sphere({ parent: "antenna_l", at: [0, 0.14, 0], radius: 0.025, material: "body" }));
  part("antenna_tip_r", sphere({ parent: "antenna_r", at: [0, 0.14, 0], radius: 0.025, material: "body" }));

  part("wing_l", box({
    parent: "body",
    at: [-0.38, 0.08, 0],
    size: [0.62, 0.025, 0.56],
    material: "wing",
    joint: { pivot: [0.31, 0, 0], axis: [0, 0, 1] },
    faces: {
      up: { texture: "wing_pattern" },
      down: { texture: "wing_pattern" },
    },
  }));
  part("wing_r", box({
    parent: "body",
    at: [0.38, 0.08, 0],
    size: [0.62, 0.025, 0.56],
    material: "wing",
    joint: { pivot: [-0.31, 0, 0], axis: [0, 0, 1] },
    faces: {
      up: { texture: "wing_pattern" },
      down: { texture: "wing_pattern" },
    },
  }));

  part("wing_l_edge", box({ parent: "wing_l", at: [-0.02, 0.018, 0], size: [0.66, 0.018, 0.04], material: "wing_edge" }));
  part("wing_r_edge", box({ parent: "wing_r", at: [0.02, 0.018, 0], size: [0.66, 0.018, 0.04], material: "wing_edge" }));
  part("spot_l1", sphere({ parent: "wing_l", at: [-0.17, 0.035, -0.16], radius: 0.045, material: "spot_blue" }));
  part("spot_l2", sphere({ parent: "wing_l", at: [-0.35, 0.035, 0.14], radius: 0.04, material: "spot_yellow" }));
  part("spot_r1", sphere({ parent: "wing_r", at: [0.17, 0.035, -0.16], radius: 0.045, material: "spot_blue" }));
  part("spot_r2", sphere({ parent: "wing_r", at: [0.35, 0.035, 0.14], radius: 0.04, material: "spot_yellow" }));

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
    leftWing: "wing_l",
    rightWing: "wing_r",
  });
});
