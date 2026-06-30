import { figure } from "../../src/dsl";

export default figure("chicken", ({ mat, part, box, capsule, sphere, cylinder, bipedWalk, swing, followThrough }) => {
  mat("feather", "#f3ede2");
  mat("feather_shade", "#d9cfbd");
  mat("comb", "#cf3a32");
  mat("wattle", "#b8302a");
  mat("beak", "#f2a531");
  mat("leg", "#e8a23a");
  mat("foot", "#d98c25");
  mat("eye", "#1c1712");

  // Plump egg-shaped body, built from spheres so it stays unrotated and
  // "down" is truly down for every child (legs, neck, tail). Raised so the
  // thin legs are clearly visible above y=0.
  part("body", sphere({ at: [0, 0.74, 0.05], radius: 0.33, material: "feather" }));
  // Rear sphere extends the body backward into a chicken egg shape.
  part("rump", sphere({ parent: "body", at: [0, 0.02, 0.27], radius: 0.27, material: "feather" }));
  // Breast bulges forward and down at the front of the body.
  part("breast", sphere({ parent: "body", at: [0, -0.08, -0.22], radius: 0.24, material: "feather" }));
  // Lower belly shading sphere to round out the underside.
  part("belly", sphere({ parent: "body", at: [0, -0.16, 0.04], radius: 0.26, material: "feather_shade" }));

  // Short neck rising up and forward from the front-top of the body
  // (-z is forward). Negative X rotation tilts the top toward the front.
  part("neck", capsule({
    parent: "body",
    at: [0, 0.34, -0.26],
    rot: [-22, 0, 0],
    radius: 0.13,
    length: 0.22,
    material: "feather",
  }));

  // Small head at the top of the neck, sitting forward of the body.
  part("head", sphere({
    parent: "neck",
    at: [0, 0.24, -0.06],
    radius: 0.18,
    material: "feather",
  }));
  part("eye_l", sphere({ parent: "head", at: [-0.13, 0.05, -0.09], radius: 0.03, material: "eye" }));
  part("eye_r", sphere({ parent: "head", at: [0.13, 0.05, -0.09], radius: 0.03, material: "eye" }));

  // Red comb: a row of rounded crests on top of the head.
  part("comb_mid", sphere({ parent: "head", at: [0, 0.18, -0.02], radius: 0.075, material: "comb" }));
  part("comb_front", sphere({ parent: "head", at: [0, 0.15, -0.12], radius: 0.06, material: "comb" }));
  part("comb_back", sphere({ parent: "head", at: [0, 0.15, 0.09], radius: 0.06, material: "comb" }));

  // Beak: small orange wedge pointing forward.
  part("beak", cylinder({
    parent: "head",
    at: [0, -0.02, -0.2],
    rot: [-90, 0, 0],
    radiusTop: 0.005,
    radiusBottom: 0.075,
    length: 0.14,
    radialSegments: 8,
    material: "beak",
  }));

  // Wattle: red dangle under the beak.
  part("wattle", sphere({ parent: "head", at: [0, -0.16, -0.13], radius: 0.055, material: "wattle" }));

  // Two small wings folded on the sides of the body.
  part("wing_l", box({
    parent: "body",
    at: [-0.31, 0.06, 0.0],
    size: [0.07, 0.3, 0.46],
    material: "feather_shade",
    joint: { pivot: [0, 0.12, -0.18], axis: [0, 0, 1] },
  }));
  part("wing_r", box({
    parent: "body",
    at: [0.31, 0.06, 0.0],
    size: [0.07, 0.3, 0.46],
    material: "feather_shade",
    joint: { pivot: [0, 0.12, -0.18], axis: [0, 0, 1] },
  }));

  // Fan of tail feathers at the back, angled up and back.
  part("tail", box({
    parent: "body",
    at: [0, 0.22, 0.46],
    rot: [38, 0, 0],
    size: [0.38, 0.46, 0.09],
    material: "feather",
    joint: { pivot: [0, -0.23, 0], axis: [1, 0, 0] },
  }));
  part("tail_top", box({ parent: "tail", at: [0, 0.2, 0.04], rot: [12, 0, 0], size: [0.26, 0.32, 0.07], material: "feather_shade" }));

  // Two thin yellow legs reaching down to the floor (y=0).
  part("leg_l", capsule({
    parent: "body",
    at: [-0.13, -0.43, 0.06],
    radius: 0.045,
    length: 0.5,
    material: "leg",
    joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
  }));
  part("leg_r", capsule({
    parent: "body",
    at: [0.13, -0.43, 0.06],
    radius: 0.045,
    length: 0.5,
    material: "leg",
    joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
  }));

  // Flat three-toed feet on the floor.
  part("foot_l", box({ parent: "leg_l", at: [0, -0.28, -0.04], size: [0.13, 0.05, 0.22], material: "foot" }));
  part("foot_r", box({ parent: "leg_r", at: [0, -0.28, -0.04], size: [0.13, 0.05, 0.22], material: "foot" }));

  bipedWalk("walk", {
    fps: 12,
    duration: 0.78,
    cycleDistance: 0.5,
    loop: true,
    samples: 13,
    body: "body",
    bodyBob: 0.02,
    bodyBobCenter: 0.02,
    head: "head",
    headSwingDegrees: 3,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.58,
    swingDegrees: 22,
    tracks: [
      // Subtle wing flutter, opposite sides slightly out of phase.
      swing("wing_l", { axis: "z", degrees: 7, center: -3, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -7, center: 3, frequency: 2 }),
      // Head bobs forward/back like a walking chicken.
      swing("head", { axis: "x", degrees: 8, frequency: 2 }),
      // Tail flicks a beat behind the body bob.
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.12 }),
    ],
  });
});
