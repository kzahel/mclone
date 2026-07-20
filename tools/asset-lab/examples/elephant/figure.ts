import { figure } from "../../src/dsl";

export default figure("elephant", ({
  mat,
  part,
  box,
  capsule,
  sphere,
  cylinder,
  quadrupedWalk,
  swing,
  followThrough,
}) => {
  mat("hide", "#7f8588");
  mat("hide_light", "#989d9f");
  mat("hide_shadow", "#656b6e");
  mat("ear_inner", "#a17f82");
  mat("tusk", "#eee5c7");
  mat("eye", "#171819");
  mat("toenail", "#d8d0b8");

  // A small root carries whole-body animation while remaining hidden inside a
  // single lengthwise capsule. This produces one continuous rounded barrel and
  // keeps the coordinate frame neutral for the head, legs, and tail.
  part("body", sphere({
    at: [0, 1.32, 0.08],
    radius: 0.12,
    widthSegments: 10,
    heightSegments: 6,
    material: "hide",
  }));
  part("body_barrel", capsule({
    parent: "body",
    rot: [90, 0, 0],
    radius: 0.56,
    length: 0.95,
    capSegments: 8,
    radialSegments: 16,
    material: "hide",
  }));

  // The head sits low at the front of the shoulders. Its children inherit the
  // walk's body bob and head motion before applying their own local animation.
  part("head", sphere({
    parent: "body",
    at: [0, 0.24, -0.94],
    radius: 0.5,
    widthSegments: 18,
    heightSegments: 10,
    material: "hide",
    joint: { pivot: [0, 0.03, 0.34], axis: [1, 0, 0] },
  }));
  part("forehead", sphere({
    parent: "head",
    at: [0, 0.19, -0.27],
    radius: 0.34,
    widthSegments: 16,
    heightSegments: 8,
    material: "hide_light",
  }));

  // Broad twelve-sided ear discs hinge at their inner edges. Each inset is a
  // smaller disc parented to the moving outer ear and nudged toward the front.
  part("ear_l", cylinder({
    parent: "head",
    at: [-0.47, 0.04, 0.05],
    rot: [90, 0, -5],
    radius: 0.42,
    length: 0.07,
    radialSegments: 12,
    material: "hide",
    joint: { pivot: [0.33, 0, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", cylinder({
    parent: "head",
    at: [0.47, 0.04, 0.05],
    rot: [90, 0, 5],
    radius: 0.42,
    length: 0.07,
    radialSegments: 12,
    material: "hide",
    joint: { pivot: [-0.33, 0, 0], axis: [1, 0, 0] },
  }));
  part("ear_inner_l", cylinder({
    parent: "ear_l",
    at: [-0.015, -0.05, 0],
    radius: 0.32,
    length: 0.025,
    radialSegments: 12,
    material: "ear_inner",
  }));
  part("ear_inner_r", cylinder({
    parent: "ear_r",
    at: [0.015, -0.05, 0],
    radius: 0.32,
    length: 0.025,
    radialSegments: 12,
    material: "ear_inner",
  }));

  part("eye_l", sphere({
    parent: "head",
    at: [-0.32, 0.1, -0.38],
    radius: 0.035,
    widthSegments: 10,
    heightSegments: 6,
    material: "eye",
  }));
  part("eye_r", sphere({
    parent: "head",
    at: [0.32, 0.1, -0.38],
    radius: 0.035,
    widthSegments: 10,
    heightSegments: 6,
    material: "eye",
  }));

  // The trunk is a tapered articulated chain. Each segment pivots from its top
  // and bends a little farther than its parent, producing a soft traveling arc
  // rather than making the whole trunk swing as one rigid pendulum.
  part("trunk_1", capsule({
    parent: "head",
    at: [0, -0.38, -0.37],
    rot: [3, 0, 0],
    radius: 0.15,
    length: 0.36,
    capSegments: 5,
    radialSegments: 12,
    material: "hide_light",
    joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
  }));
  part("trunk_2", capsule({
    parent: "trunk_1",
    at: [0, -0.43, -0.02],
    rot: [7, 0, 0],
    radius: 0.12,
    length: 0.32,
    capSegments: 5,
    radialSegments: 12,
    material: "hide_light",
    joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] },
  }));
  part("trunk_3", capsule({
    parent: "trunk_2",
    at: [0, -0.37, -0.045],
    rot: [14, 0, 0],
    radius: 0.09,
    length: 0.27,
    capSegments: 5,
    radialSegments: 12,
    material: "hide_light",
    joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] },
  }));
  part("trunk_tip", sphere({
    parent: "trunk_3",
    at: [0, -0.25, -0.075],
    radius: 0.095,
    widthSegments: 12,
    heightSegments: 7,
    material: "hide_light",
  }));

  // Tapered tusks angle down and forward beside the trunk.
  part("tusk_l", cylinder({
    parent: "head",
    at: [-0.23, -0.3, -0.43],
    rot: [27, 0, -5],
    radiusTop: 0.07,
    radiusBottom: 0.012,
    length: 0.45,
    radialSegments: 10,
    material: "tusk",
  }));
  part("tusk_r", cylinder({
    parent: "head",
    at: [0.23, -0.3, -0.43],
    rot: [27, 0, 5],
    radiusTop: 0.07,
    radiusBottom: 0.012,
    length: 0.45,
    radialSegments: 10,
    material: "tusk",
  }));

  // Thick column legs and wide feet carry the body's visual weight. Each foot
  // has three pale front nails, an inexpensive detail that reads at a distance.
  for (const [suffix, x, z] of [
    ["fl", -0.42, -0.5],
    ["fr", 0.42, -0.5],
    ["bl", -0.42, 0.53],
    ["br", 0.42, 0.53],
  ] as const) {
    part(`leg_${suffix}`, capsule({
      parent: "body",
      at: [x, -0.7, z],
      radius: 0.15,
      length: 0.66,
      capSegments: 4,
      radialSegments: 10,
      material: "hide",
      joint: { pivot: [0, 0.45, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.47, -0.06],
      size: [0.4, 0.18, 0.48],
      material: "hide_shadow",
    }));
    for (const [toeIndex, toeX] of [-0.115, 0, 0.115].entries()) {
      part(`toenail_${suffix}_${toeIndex + 1}`, box({
        parent: `foot_${suffix}`,
        at: [toeX, -0.005, -0.247],
        size: [0.075, 0.08, 0.025],
        material: "toenail",
      }));
    }
  }

  part("tail", capsule({
    parent: "body",
    at: [0, -0.18, 0.88],
    rot: [-7, 0, 0],
    radius: 0.045,
    length: 0.58,
    capSegments: 4,
    radialSegments: 8,
    material: "hide_shadow",
    joint: { pivot: [0, 0.33, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", sphere({
    parent: "tail",
    at: [0, -0.37, 0.03],
    radius: 0.09,
    widthSegments: 10,
    heightSegments: 6,
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
      // A small nod is independent of the gait macro's side-to-side head turn.
      swing("head", { axis: "x", degrees: 1.8, phase: 0.5 }),

      // Independent phase offsets make the trunk's bend travel toward the tip.
      swing("trunk_1", { axis: "x", degrees: 4, phase: 0.04 }),
      swing("trunk_2", { axis: "x", degrees: 5.5, phase: 0.11 }),
      swing("trunk_3", { axis: "x", degrees: 7, phase: 0.19 }),
      swing("trunk_1", { axis: "z", degrees: 2, phase: 0.3 }),

      // Body-driven lag layers over the trunk's own swing and gives it weight.
      followThrough("trunk_1", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 2.5, lag: 0.1, overshoot: 0.45 }),
      followThrough("trunk_2", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, lag: 0.16, overshoot: 0.6 }),
      followThrough("trunk_3", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5.5, lag: 0.22, overshoot: 0.75 }),

      // Ears answer the shoulder rise with a restrained flap rather than flying.
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3.5, lag: 0.13, overshoot: 0.4 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3.5, lag: 0.13, overshoot: 0.4 }),
    ],
  });
});
