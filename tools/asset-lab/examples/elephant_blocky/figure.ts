import { figure } from "../../src/dsl";

// A deliberately box-only elephant study. The figure keeps the sparse visual
// grammar of Minecraft quadrupeds: one body cuboid, one head cuboid, four
// pivoted legs, and a small number of child boxes for identifying features.
export default figure("elephant_blocky", ({
  mat,
  part,
  box,
  quadrupedWalk,
  swing,
  followThrough,
}) => {
  mat("hide", "#747b7e");
  mat("hide_light", "#8d9497");
  mat("hide_shadow", "#596064");
  mat("ear_inner", "#98787c");
  mat("tusk", "#eee4c5");
  mat("eye", "#151718");
  mat("toenail", "#d8d0b7");

  // The torso is intentionally a single prism, not a stack of cubes trying to
  // fake a curve. Its long horizontal silhouette matches vanilla quadrupeds.
  part("body", box({
    at: [0, 1.25, 0.05],
    size: [1.24, 0.9, 1.62],
    material: "hide",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.43, 0.04],
    size: [1.06, 0.12, 1.34],
    material: "hide_shadow",
  }));

  part("head", box({
    parent: "body",
    at: [0, 0.2, -1.02],
    size: [0.92, 0.78, 0.68],
    material: "hide_light",
    joint: { pivot: [0, 0.02, 0.3], axis: [1, 0, 0] },
  }));
  part("brow", box({
    parent: "head",
    at: [0, 0.28, -0.315],
    size: [0.76, 0.2, 0.08],
    material: "hide",
  }));

  // Thin ear slabs sit behind the head. Their inner-edge pivots allow a small
  // secondary flap while retaining a deliberately rectangular outline.
  part("ear_l", box({
    parent: "head",
    at: [-0.57, 0.02, 0.04],
    rot: [0, -5, -4],
    size: [0.62, 0.72, 0.08],
    material: "hide",
    joint: { pivot: [0.28, 0, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.57, 0.02, 0.04],
    rot: [0, 5, 4],
    size: [0.62, 0.72, 0.08],
    material: "hide",
    joint: { pivot: [-0.28, 0, 0], axis: [1, 0, 0] },
  }));
  part("ear_inner_l", box({
    parent: "ear_l",
    at: [-0.015, 0, -0.052],
    size: [0.46, 0.56, 0.025],
    material: "ear_inner",
  }));
  part("ear_inner_r", box({
    parent: "ear_r",
    at: [0.015, 0, -0.052],
    size: [0.46, 0.56, 0.025],
    material: "ear_inner",
  }));

  part("eye_l", box({
    parent: "head",
    at: [-0.26, 0.08, -0.355],
    size: [0.075, 0.075, 0.035],
    material: "eye",
  }));
  part("eye_r", box({
    parent: "head",
    at: [0.26, 0.08, -0.355],
    size: [0.075, 0.075, 0.035],
    material: "eye",
  }));

  // Three shrinking prisms form a stepped trunk. The gaps are hidden through
  // overlap, while the separate top pivots retain the traveling bend.
  part("trunk_1", box({
    parent: "head",
    at: [0, -0.45, -0.3],
    rot: [2, 0, 0],
    size: [0.3, 0.52, 0.3],
    material: "hide_light",
    joint: { pivot: [0, 0.26, 0], axis: [1, 0, 0] },
  }));
  part("trunk_2", box({
    parent: "trunk_1",
    at: [0, -0.43, -0.025],
    rot: [6, 0, 0],
    size: [0.25, 0.42, 0.25],
    material: "hide_light",
    joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
  }));
  part("trunk_3", box({
    parent: "trunk_2",
    at: [0, -0.35, -0.045],
    rot: [11, 0, 0],
    size: [0.2, 0.34, 0.2],
    material: "hide_light",
    joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
  }));
  part("trunk_tip", box({
    parent: "trunk_3",
    at: [0, -0.24, -0.055],
    size: [0.24, 0.16, 0.24],
    material: "hide_light",
  }));

  // A pair of two-box tusks gives the otherwise blunt cuboid vocabulary a
  // stepped taper. They remain visibly block-made even in three-quarter view.
  part("tusk_l", box({
    parent: "head",
    at: [-0.24, -0.31, -0.39],
    rot: [25, 0, -4],
    size: [0.085, 0.34, 0.085],
    material: "tusk",
  }));
  part("tusk_l_tip", box({
    parent: "tusk_l",
    at: [0, -0.23, -0.06],
    rot: [8, 0, 0],
    size: [0.055, 0.2, 0.055],
    material: "tusk",
  }));
  part("tusk_r", box({
    parent: "head",
    at: [0.24, -0.31, -0.39],
    rot: [25, 0, 4],
    size: [0.085, 0.34, 0.085],
    material: "tusk",
  }));
  part("tusk_r_tip", box({
    parent: "tusk_r",
    at: [0, -0.23, -0.06],
    rot: [8, 0, 0],
    size: [0.055, 0.2, 0.055],
    material: "tusk",
  }));

  // Vanilla-style straight cuboid legs carry wide block feet. Three small
  // nail blocks per foot reinforce the elephant read without rounding it.
  for (const [suffix, x, z] of [
    ["fl", -0.43, -0.52],
    ["fr", 0.43, -0.52],
    ["bl", -0.43, 0.54],
    ["br", 0.43, 0.54],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.75, z],
      size: [0.27, 0.72, 0.29],
      material: "hide",
      joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.43, -0.065],
      size: [0.39, 0.16, 0.46],
      material: "hide_shadow",
    }));
    for (const [toeIndex, toeX] of [-0.115, 0, 0.115].entries()) {
      part(`toenail_${suffix}_${toeIndex + 1}`, box({
        parent: `foot_${suffix}`,
        at: [toeX, -0.01, -0.242],
        size: [0.075, 0.075, 0.025],
        material: "toenail",
      }));
    }
  }

  part("tail", box({
    parent: "body",
    at: [0, -0.16, 0.88],
    rot: [-6, 0, 0],
    size: [0.075, 0.58, 0.075],
    material: "hide_shadow",
    joint: { pivot: [0, 0.29, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.37, 0.025],
    size: [0.16, 0.18, 0.14],
    material: "hide_shadow",
  }));

  quadrupedWalk("walk", {
    fps: 24,
    duration: 1.6,
    cycleDistance: 0.9,
    gait: "walk",
    loop: true,
    samples: 25,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.018,
    bodyBobCenter: 0.018,
    head: "head",
    headSwingDegrees: 1.4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.7,
    swingDegrees: 13,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      swing("head", { axis: "x", degrees: 1.5, phase: 0.5 }),
      swing("trunk_1", { axis: "x", degrees: 3.5, phase: 0.04 }),
      swing("trunk_2", { axis: "x", degrees: 5, phase: 0.11 }),
      swing("trunk_3", { axis: "x", degrees: 6.5, phase: 0.19 }),
      swing("trunk_1", { axis: "z", degrees: 1.5, phase: 0.3 }),
      followThrough("trunk_1", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 2, lag: 0.1, overshoot: 0.4 }),
      followThrough("trunk_2", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3.5, lag: 0.16, overshoot: 0.55 }),
      followThrough("trunk_3", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, lag: 0.22, overshoot: 0.7 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, lag: 0.13, overshoot: 0.35 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, lag: 0.13, overshoot: 0.35 }),
    ],
  });
});
