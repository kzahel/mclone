import { legacyFigure } from "../../src/dsl";

export default legacyFigure("horse_rounded", ({ mat, asciiTexture, part, box, capsule, sphere, cylinder, quadrupedWalk, followThrough }) => {
  // Bay coloring: warm brown body, black "points" (mane, tail, lower legs).
  mat("coat", "#8a5a2b");
  mat("coat_shadow", "#724824");
  mat("coat_light", "#a06d36");
  mat("muzzle", "#5b3a1c");
  mat("nostril", "#2a1a0d");
  mat("point", "#1a140f"); // black mane/tail/lower-leg points
  mat("hoof", "#171210");
  mat("ear_inner", "#6e4a26");
  mat("blaze", "#e9e3d6"); // white facial blaze
  mat("eye", "#120c08");

  // Face: dark horse eyes set wide, with a white blaze running down the center.
  asciiTexture("face", {
    palette: {
      ".": "#8a5a2b",
      "e": "#120c08",
      "w": "#e9e3d6",
    },
    pixels: [
      "...ww...",
      "ee.ww.ee",
      "ee.ww.ee",
      "...ww...",
      "...ww...",
      "..www...",
      "..www...",
      "..www...",
    ],
  });

  // Two nostrils on the muzzle front.
  asciiTexture("muzzle_face", {
    palette: {
      ".": "#5b3a1c",
      "n": "#2a1a0d",
    },
    pixels: [
      "........",
      "........",
      "........",
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "........",
      "........",
    ],
  });

  // Long, fairly slim, deep-chested barrel. Center at origin; legs hang below.
  // Shallower than a cow so more leg shows and it reads as leggy.
  part("body", box({ size: [0.74, 0.6, 1.74], material: "coat" }));
  // Slightly lighter underbelly.
  part("belly", box({ parent: "body", at: [0, -0.3, 0.04], size: [0.62, 0.08, 1.24], material: "coat_shadow" }));
  // Rounded haunch over the rear legs (rump muscle).
  part("haunch", box({ parent: "body", at: [0, 0.06, 0.66], size: [0.78, 0.6, 0.46], material: "coat_light" }));

  // Long neck raised up-and-forward toward the head (the key horse posture cue).
  // Base overlaps into the front of the barrel so it stays attached.
  part("neck", box({
    parent: "body",
    at: [0, 0.38, -0.82],
    rot: [-42, 0, 0],
    size: [0.4, 0.82, 0.44],
    material: "coat",
    joint: { pivot: [0, -0.41, 0], axis: [1, 0, 0] },
  }));

  // Head carried forward at the top of the neck, with a long muzzle.
  part("head", box({
    parent: "neck",
    at: [0, 0.5, -0.18],
    rot: [30, 0, 0],
    size: [0.34, 0.4, 0.52],
    material: "coat",
    faces: {
      north: { texture: "face" },
    },
  }));
  // Long tapering muzzle (horse face is long and narrow).
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.42],
    size: [0.26, 0.26, 0.34],
    material: "muzzle",
    faces: {
      north: { texture: "muzzle_face" },
    },
  }));
  part("nose", sphere({ parent: "muzzle", at: [0, 0.0, -0.18], radius: 0.05, material: "nostril" }));

  // Upright, alert ears.
  part("ear_l", box({ parent: "head", at: [-0.12, 0.26, 0.12], rot: [-6, 0, -10], size: [0.1, 0.22, 0.08], material: "coat" }));
  part("ear_r", box({ parent: "head", at: [0.12, 0.26, 0.12], rot: [-6, 0, 10], size: [0.1, 0.22, 0.08], material: "coat" }));
  part("ear_l_inner", box({ parent: "ear_l", at: [0, 0.0, -0.04], size: [0.05, 0.14, 0.03], material: "ear_inner" }));
  part("ear_r_inner", box({ parent: "ear_r", at: [0, 0.0, -0.04], size: [0.05, 0.14, 0.03], material: "ear_inner" }));

  // Mane: a row of black tufts running along the top of the neck. Each is a
  // child of the neck so they follow the neck and can flick with follow-through.
  part("mane_1", box({ parent: "neck", at: [0, 0.34, 0.2], rot: [10, 0, 0], size: [0.1, 0.2, 0.12], material: "point" }));
  part("mane_2", box({ parent: "neck", at: [0, 0.18, 0.21], rot: [8, 0, 0], size: [0.11, 0.22, 0.12], material: "point" }));
  part("mane_3", box({ parent: "neck", at: [0, 0.0, 0.22], rot: [6, 0, 0], size: [0.12, 0.24, 0.12], material: "point" }));
  part("mane_4", box({ parent: "neck", at: [0, -0.18, 0.22], rot: [4, 0, 0], size: [0.12, 0.24, 0.12], material: "point" }));
  // Forelock tuft between the ears.
  part("forelock", box({ parent: "head", at: [0, 0.18, 0.18], rot: [22, 0, 0], size: [0.12, 0.14, 0.1], material: "point" }));

  // Long, slim legs. Body bottom is at y = -0.36. Capsule length 0.72, radius
  // 0.085 => half-height 0.445; centered at -0.42 so the cylinder bottom is at
  // about -0.78 and the hoof/sock hang just below. Pivot at the top of the leg.
  part("leg_fl", capsule({ parent: "body", at: [-0.26, -0.42, -0.58], radius: 0.085, length: 0.72, material: "coat", joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.26, -0.42, -0.58], radius: 0.085, length: 0.72, material: "coat", joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.26, -0.42, 0.62], radius: 0.09, length: 0.72, material: "coat", joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.26, -0.42, 0.62], radius: 0.09, length: 0.72, material: "coat", joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] } }));

  // Black lower-leg "socks" (the bay points) near the bottom of each leg.
  part("sock_fl", box({ parent: "leg_fl", at: [0, -0.26, 0], size: [0.17, 0.2, 0.17], material: "point" }));
  part("sock_fr", box({ parent: "leg_fr", at: [0, -0.26, 0], size: [0.17, 0.2, 0.17], material: "point" }));
  part("sock_bl", box({ parent: "leg_bl", at: [0, -0.26, 0], size: [0.18, 0.2, 0.18], material: "point" }));
  part("sock_br", box({ parent: "leg_br", at: [0, -0.26, 0], size: [0.18, 0.2, 0.18], material: "point" }));

  // Single hooves at the very bottom of each leg.
  part("hoof_fl", box({ parent: "leg_fl", at: [0, -0.4, 0.01], size: [0.18, 0.1, 0.2], material: "hoof" }));
  part("hoof_fr", box({ parent: "leg_fr", at: [0, -0.4, 0.01], size: [0.18, 0.1, 0.2], material: "hoof" }));
  part("hoof_bl", box({ parent: "leg_bl", at: [0, -0.4, 0.01], size: [0.19, 0.1, 0.2], material: "hoof" }));
  part("hoof_br", box({ parent: "leg_br", at: [0, -0.4, 0.01], size: [0.19, 0.1, 0.2], material: "hoof" }));

  // Long flowing tail: docks off the rump and falls mostly straight down with a
  // slight backward set, plus a thicker skirt of hair below so it reads as a
  // full horse tail rather than a stub. Small rotation keeps it hanging, not
  // sticking out like a fifth leg.
  part("tail", capsule({
    parent: "body",
    at: [0, 0.2, 0.86],
    rot: [20, 0, 0],
    radius: 0.07,
    length: 0.66,
    material: "point",
    joint: { pivot: [0, 0.33, 0], axis: [0, 0, 1] },
  }));
  part("tail_skirt", capsule({
    parent: "tail",
    at: [0, -0.52, -0.04],
    rot: [-12, 0, 0],
    radius: 0.09,
    length: 0.52,
    material: "point",
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.0,
    cycleDistance: 1.05,
    gait: "walk",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.64,
    swingDegrees: 19,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      // Ears flick a beat behind the body bob.
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.12 }),
      // Mane lifts and settles trailing the body bob, more toward the withers.
      followThrough("mane_1", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.14 }),
      followThrough("mane_2", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.7, lag: 0.16 }),
      followThrough("mane_3", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.75, lag: 0.18 }),
      followThrough("mane_4", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 11, overshoot: 0.75, lag: 0.2 }),
      // Tail trails the body vertically on top of its own side-to-side swing.
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.8, lag: 0.2 }),
    ],
  });
});
