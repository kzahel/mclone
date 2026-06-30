import { figure } from "../../src/dsl";

export default figure("cat", ({ mat, asciiTexture, part, box, capsule, sphere, cylinder, quadrupedWalk, followThrough }) => {
  mat("fur", "#5f6570");
  mat("fur_dark", "#303743");
  mat("fur_light", "#b9bec5");
  mat("eye", "#8bd05b");
  mat("nose", "#d08a9b");
  mat("paw", "#252932");

  asciiTexture("face", {
    palette: {
      ".": "#5f6570",
      "e": "#8bd05b",
      "n": "#d08a9b",
      "w": "#f2f4f8",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "..w..w..",
      "...nn...",
      "........",
      "........",
    ],
  });

  part("body", box({ size: [0.9, 0.42, 0.94], material: "fur" }));
  part("belly", box({ parent: "body", at: [0, -0.08, -0.02], size: [0.72, 0.22, 0.74], material: "fur_light" }));
  part("head", box({
    parent: "body",
    at: [0, 0.12, -0.64],
    size: [0.5, 0.42, 0.42],
    material: "fur",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("muzzle", box({ parent: "head", at: [0, -0.08, -0.27], size: [0.3, 0.14, 0.12], material: "fur_light" }));
  part("nose", sphere({ parent: "muzzle", at: [0, 0.02, -0.08], radius: 0.035, material: "nose" }));
  part("ear_l", box({ parent: "head", at: [-0.19, 0.28, -0.03], rot: [0, 0, -22], size: [0.13, 0.24, 0.09], material: "fur_dark" }));
  part("ear_r", box({ parent: "head", at: [0.19, 0.28, -0.03], rot: [0, 0, 22], size: [0.13, 0.24, 0.09], material: "fur_dark" }));

  part("whisker_l1", cylinder({ parent: "muzzle", at: [-0.22, 0.02, -0.04], rot: [0, 0, 82], radius: 0.009, length: 0.34, radialSegments: 6, material: "fur_light" }));
  part("whisker_l2", cylinder({ parent: "muzzle", at: [-0.22, -0.04, -0.04], rot: [0, 0, 96], radius: 0.009, length: 0.3, radialSegments: 6, material: "fur_light" }));
  part("whisker_r1", cylinder({ parent: "muzzle", at: [0.22, 0.02, -0.04], rot: [0, 0, -82], radius: 0.009, length: 0.34, radialSegments: 6, material: "fur_light" }));
  part("whisker_r2", cylinder({ parent: "muzzle", at: [0.22, -0.04, -0.04], rot: [0, 0, -96], radius: 0.009, length: 0.3, radialSegments: 6, material: "fur_light" }));

  part("leg_fl", capsule({ parent: "body", at: [-0.29, -0.39, -0.28], radius: 0.055, length: 0.38, material: "fur", joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.29, -0.39, -0.28], radius: 0.055, length: 0.38, material: "fur", joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.29, -0.39, 0.3], radius: 0.06, length: 0.4, material: "fur_dark", joint: { pivot: [0, 0.2, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.29, -0.39, 0.3], radius: 0.06, length: 0.4, material: "fur_dark", joint: { pivot: [0, 0.2, 0], axis: [1, 0, 0] } }));

  part("paw_fl", box({ parent: "leg_fl", at: [0, -0.24, -0.04], size: [0.13, 0.06, 0.17], material: "paw" }));
  part("paw_fr", box({ parent: "leg_fr", at: [0, -0.24, -0.04], size: [0.13, 0.06, 0.17], material: "paw" }));
  part("paw_bl", box({ parent: "leg_bl", at: [0, -0.25, 0.02], size: [0.13, 0.06, 0.17], material: "paw" }));
  part("paw_br", box({ parent: "leg_br", at: [0, -0.25, 0.02], size: [0.13, 0.06, 0.17], material: "paw" }));

  part("tail", capsule({
    parent: "body",
    at: [0, 0.27, 0.55],
    rot: [62, 0, 0],
    radius: 0.045,
    length: 0.62,
    material: "fur_dark",
    joint: { pivot: [0, -0.31, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 0.82,
    cycleDistance: 0.78,
    gait: "walk",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.009,
    bodyBobCenter: 0.011,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.62,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 10,
    tracks: [
      // Ears flop a beat behind the body bob, with a little overshoot.
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 15, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 15, overshoot: 0.6, lag: 0.12 }),
      // Tail trails the body vertically on top of its own side-to-side swing.
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 11, overshoot: 0.75, lag: 0.18 }),
    ],
  });
});
