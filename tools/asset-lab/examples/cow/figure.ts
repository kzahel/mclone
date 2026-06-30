import { figure } from "../../src/dsl";

export default figure("cow", ({ mat, asciiTexture, part, box, capsule, sphere, cylinder, quadrupedWalk, followThrough }) => {
  // Holstein black-and-white.
  mat("hide", "#f2f1ee");
  mat("spot", "#1d1b19");
  mat("hide_shadow", "#d8d6d1");
  mat("muzzle", "#caa6a8");
  mat("nostril", "#3a2c2d");
  mat("horn", "#d9cfb6");
  mat("hoof", "#26201d");
  mat("ear_inner", "#b58a8c");
  mat("udder", "#e7b1b0");
  mat("teat", "#d99a99");
  mat("tail_tuft", "#1d1b19");
  mat("eye", "#16110f");

  // Face: big dark cow eyes.
  asciiTexture("face", {
    palette: {
      ".": "#f2f1ee",
      "e": "#16110f",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  });

  // Broad muzzle face: two nostrils.
  asciiTexture("muzzle_face", {
    palette: {
      ".": "#caa6a8",
      "n": "#3a2c2d",
    },
    pixels: [
      "........",
      "........",
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "........",
      "........",
      "........",
    ],
  });

  // Large boxy barrel body. Center at origin; legs hang below.
  part("body", box({ size: [1.0, 0.86, 1.62], material: "hide" }));
  // Holstein patches: irregular black blobs on the body using distinct parts.
  part("spot_back", box({ parent: "body", at: [0.12, 0.46, 0.18], size: [0.66, 0.12, 0.78], material: "spot" }));
  part("spot_left", box({ parent: "body", at: [-0.52, 0.06, -0.28], size: [0.06, 0.5, 0.5], material: "spot" }));
  part("spot_left_rear", box({ parent: "body", at: [-0.52, -0.14, 0.42], size: [0.06, 0.42, 0.46], material: "spot" }));
  part("spot_right", box({ parent: "body", at: [0.52, 0.1, 0.34], size: [0.06, 0.44, 0.56], material: "spot" }));
  part("spot_right_front", box({ parent: "body", at: [0.52, -0.06, -0.42], size: [0.06, 0.36, 0.4], material: "spot" }));
  part("spot_belly", box({ parent: "body", at: [-0.18, -0.44, -0.1], size: [0.5, 0.1, 0.6], material: "spot" }));
  // Lighter underside.
  part("belly", box({ parent: "body", at: [0, -0.45, 0.2], size: [0.86, 0.08, 1.0], material: "hide_shadow" }));

  // Neck angles down-forward toward the head.
  part("neck", box({ parent: "body", at: [0, 0.18, -0.86], rot: [18, 0, 0], size: [0.58, 0.5, 0.5], material: "hide" }));
  part("neck_spot", box({ parent: "neck", at: [0.0, 0.16, -0.12], size: [0.42, 0.16, 0.42], material: "spot" }));

  // Head with broad muzzle.
  part("head", box({
    parent: "neck",
    at: [0, 0.18, -0.46],
    size: [0.56, 0.56, 0.5],
    material: "hide",
    faces: {
      north: { texture: "face" },
    },
  }));
  // Asymmetric black poll/forehead patch (sits behind the eyes, over the top of the head).
  part("head_patch", box({ parent: "head", at: [-0.08, 0.26, 0.06], size: [0.36, 0.12, 0.42], material: "spot" }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.18, -0.32],
    size: [0.46, 0.26, 0.22],
    material: "muzzle",
    faces: {
      north: { texture: "muzzle_face" },
    },
  }));

  // Large ears stick out sideways, overlapping the head so they stay attached.
  part("ear_l", box({ parent: "head", at: [-0.31, 0.08, 0.06], rot: [0, 0, -24], size: [0.22, 0.14, 0.14], material: "hide" }));
  part("ear_r", box({ parent: "head", at: [0.31, 0.08, 0.06], rot: [0, 0, 24], size: [0.22, 0.14, 0.14], material: "hide" }));
  part("ear_l_inner", box({ parent: "ear_l", at: [-0.04, 0.0, -0.06], size: [0.12, 0.08, 0.04], material: "ear_inner" }));
  part("ear_r_inner", box({ parent: "ear_r", at: [0.04, 0.0, -0.06], size: [0.12, 0.08, 0.04], material: "ear_inner" }));

  // Short curved-ish horns (slight outward/upward tilt) on top of the head.
  part("horn_l", cylinder({ parent: "head", at: [-0.17, 0.32, -0.08], rot: [0, 0, 34], radiusTop: 0.03, radiusBottom: 0.07, length: 0.2, radialSegments: 8, material: "horn" }));
  part("horn_r", cylinder({ parent: "head", at: [0.17, 0.32, -0.08], rot: [0, 0, -34], radiusTop: 0.03, radiusBottom: 0.07, length: 0.2, radialSegments: 8, material: "horn" }));
  part("horn_l_tip", sphere({ parent: "horn_l", at: [0, 0.1, 0], radius: 0.03, material: "horn" }));
  part("horn_r_tip", sphere({ parent: "horn_r", at: [0, 0.1, 0], radius: 0.03, material: "horn" }));

  // Sturdy legs. Body bottom is at y = -0.43; legs hang from the body.
  // length 0.58, pivot at top (+0.29), leg center ~ -0.45 => bottom at -0.74; hoof below.
  part("leg_fl", capsule({ parent: "body", at: [-0.34, -0.46, -0.56], radius: 0.11, length: 0.58, material: "hide", joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.34, -0.46, -0.56], radius: 0.11, length: 0.58, material: "hide", joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.34, -0.46, 0.58], radius: 0.12, length: 0.58, material: "hide", joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.34, -0.46, 0.58], radius: 0.12, length: 0.58, material: "hide", joint: { pivot: [0, 0.29, 0], axis: [1, 0, 0] } }));

  // Dark lower-leg "socks" + hooves so the legs read as cow legs.
  part("sock_fl", box({ parent: "leg_fl", at: [0, -0.24, 0], size: [0.2, 0.12, 0.2], material: "spot" }));
  part("sock_fr", box({ parent: "leg_fr", at: [0, -0.24, 0], size: [0.2, 0.12, 0.2], material: "spot" }));
  part("sock_bl", box({ parent: "leg_bl", at: [0, -0.24, 0], size: [0.2, 0.12, 0.2], material: "spot" }));
  part("sock_br", box({ parent: "leg_br", at: [0, -0.24, 0], size: [0.2, 0.12, 0.2], material: "spot" }));

  part("hoof_fl", box({ parent: "leg_fl", at: [0, -0.33, 0], size: [0.22, 0.1, 0.22], material: "hoof" }));
  part("hoof_fr", box({ parent: "leg_fr", at: [0, -0.33, 0], size: [0.22, 0.1, 0.22], material: "hoof" }));
  part("hoof_bl", box({ parent: "leg_bl", at: [0, -0.33, 0], size: [0.22, 0.1, 0.22], material: "hoof" }));
  part("hoof_br", box({ parent: "leg_br", at: [0, -0.33, 0], size: [0.22, 0.1, 0.22], material: "hoof" }));

  // Udder under the rear belly.
  part("udder", sphere({ parent: "body", at: [0, -0.46, 0.42], radius: 0.18, material: "udder" }));
  part("teat_l", cylinder({ parent: "udder", at: [-0.07, -0.16, 0.02], radius: 0.025, length: 0.08, radialSegments: 6, material: "teat" }));
  part("teat_r", cylinder({ parent: "udder", at: [0.07, -0.16, 0.02], radius: 0.025, length: 0.08, radialSegments: 6, material: "teat" }));

  // Thin tail draping down the back of the rump with a dark switch/tuft.
  // Base sits at the top-rear of the body and the rope hangs down-and-back so
  // it reads as a distinct line behind the rump in profile.
  part("tail", capsule({
    parent: "body",
    at: [0, 0.18, 0.92],
    rot: [28, 0, 0],
    radius: 0.045,
    length: 0.82,
    material: "hide",
    joint: { pivot: [0, 0.41, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", sphere({ parent: "tail", at: [0, -0.46, 0], radius: 0.085, material: "tail_tuft" }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.1,
    cycleDistance: 0.82,
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
    stanceRatio: 0.66,
    swingDegrees: 15,
    tail: "tail",
    tailSwingDegrees: 9,
    tracks: [
      // Ears flop a beat behind the body bob.
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.6, lag: 0.12 }),
      // Tail trails the body vertically on top of its own swing.
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.7, lag: 0.18 }),
    ],
  });
});
