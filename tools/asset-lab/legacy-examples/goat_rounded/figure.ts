import { legacyFigure } from "../../src/dsl";

// Billy goat: mid-size quadruped, similar build to the sheep but with a short
// coat (no wool fluff), slim cloven-hoofed legs, a roughly rectangular head
// with a flat straight muzzle, swept-back horns, a chin beard, sideways floppy
// ears, and a short upturned tail. White/tan reads clearly as a goat.
export default legacyFigure("goat_rounded", ({ mat, asciiTexture, part, box, capsule, sphere, cylinder, quadrupedWalk, followThrough }) => {
  mat("coat", "#e7e0d2");        // creamy white-tan short coat
  mat("coat_shadow", "#cdc4b1"); // faint underside / saddle shading
  mat("skin", "#d9cfbb");        // muzzle/ear skin
  mat("muzzle", "#c9bda6");      // flat muzzle face
  mat("nostril", "#3a2f26");
  mat("ear_inner", "#b8a98f");
  mat("horn", "#b9a888");        // tan keratin horns
  mat("hoof", "#2b2420");
  mat("beard", "#8d7a5f");       // darker tan beard tuft
  mat("eye", "#16110f");

  // Face: alert dark goat eyes set wide on a pale head.
  asciiTexture("face", {
    palette: {
      ".": "#e7e0d2",
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

  // Flat muzzle front with two small nostrils.
  asciiTexture("muzzle_face", {
    palette: {
      ".": "#c9bda6",
      "n": "#3a2f26",
    },
    pixels: [
      "........",
      "........",
      "..n..n..",
      "..n..n..",
      "........",
      "........",
      "........",
      "........",
    ],
  });

  // Slim, slightly long barrel body. Center at origin; legs hang below.
  part("body", box({ size: [0.86, 0.62, 1.12], material: "coat" }));
  // Faint darker saddle along the back so the short coat reads with some form.
  part("saddle", box({ parent: "body", at: [0, 0.33, 0.06], size: [0.78, 0.1, 0.92], material: "coat_shadow" }));
  // Lighter-shadow underside.
  part("belly", box({ parent: "body", at: [0, -0.32, 0.04], size: [0.7, 0.08, 0.86], material: "coat_shadow" }));

  // Neck angles up-forward toward the head (goats carry the head level/alert).
  // Kept stout and overlapping the body front so there is no gap at the chest.
  part("neck", box({ parent: "body", at: [0, 0.2, -0.56], rot: [-14, 0, 0], size: [0.44, 0.46, 0.5], material: "coat" }));

  // Roughly rectangular head, longer than tall, with a flat muzzle out front.
  part("head", box({
    parent: "neck",
    at: [0, 0.14, -0.36],
    size: [0.42, 0.42, 0.48],
    material: "coat",
    faces: {
      north: { texture: "face" },
    },
  }));
  // Straight flat muzzle jutting forward off the head.
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.34],
    size: [0.3, 0.24, 0.26],
    material: "muzzle",
    faces: {
      north: { texture: "muzzle_face" },
    },
  }));

  // Sideways floppy ears that splay out from just below the horns.
  part("ear_l", box({ parent: "head", at: [-0.27, 0.12, 0.02], rot: [0, 0, -42], size: [0.26, 0.1, 0.12], material: "skin" }));
  part("ear_r", box({ parent: "head", at: [0.27, 0.12, 0.02], rot: [0, 0, 42], size: [0.26, 0.1, 0.12], material: "skin" }));
  part("ear_l_inner", box({ parent: "ear_l", at: [-0.05, 0.0, -0.05], size: [0.16, 0.06, 0.04], material: "ear_inner" }));
  part("ear_r_inner", box({ parent: "ear_r", at: [0.05, 0.0, -0.05], size: [0.16, 0.06, 0.04], material: "ear_inner" }));

  // Signature swept-back horns: a stout tapered base rising from the poll
  // (back-top of the head) and angling back along +Z, then a second segment
  // that sweeps further back over the neck to a pointed tip. This up-and-back
  // arc is the clearest goat-vs-sheep cue.
  part("horn_l", cylinder({ parent: "head", at: [-0.13, 0.24, 0.14], rot: [30, 0, 10], radiusTop: 0.045, radiusBottom: 0.085, length: 0.32, radialSegments: 8, material: "horn", joint: { pivot: [0, -0.16, 0] } }));
  part("horn_r", cylinder({ parent: "head", at: [0.13, 0.24, 0.14], rot: [30, 0, -10], radiusTop: 0.045, radiusBottom: 0.085, length: 0.32, radialSegments: 8, material: "horn", joint: { pivot: [0, -0.16, 0] } }));
  // Upper horn segment sweeps further back, parented to the base tip.
  part("horn_l_tip", cylinder({ parent: "horn_l", at: [0, 0.2, 0.05], rot: [34, 0, 0], radiusTop: 0.022, radiusBottom: 0.045, length: 0.26, radialSegments: 8, material: "horn", joint: { pivot: [0, -0.13, 0] } }));
  part("horn_r_tip", cylinder({ parent: "horn_r", at: [0, 0.2, 0.05], rot: [34, 0, 0], radiusTop: 0.022, radiusBottom: 0.045, length: 0.26, radialSegments: 8, material: "horn", joint: { pivot: [0, -0.13, 0] } }));
  part("horn_l_point", sphere({ parent: "horn_l_tip", at: [0, 0.13, 0], radius: 0.024, material: "horn" }));
  part("horn_r_point", sphere({ parent: "horn_r_tip", at: [0, 0.13, 0], radius: 0.024, material: "horn" }));

  // Pointed billy-goat beard hanging under the chin, angled slightly forward so
  // it reads clearly in profile.
  part("beard", box({
    parent: "muzzle",
    at: [0, -0.22, -0.05],
    rot: [12, 0, 0],
    size: [0.15, 0.28, 0.11],
    material: "beard",
    joint: { pivot: [0, 0.14, 0], axis: [1, 0, 0] },
  }));
  part("beard_tip", box({ parent: "beard", at: [0, -0.2, 0.0], size: [0.09, 0.16, 0.08], material: "beard" }));

  // Slim legs ending in small cloven hooves.
  // length 0.48, pivot at top (+0.24); attach at y=-0.5 -> leg top -0.26 (into
  // the body bottom at -0.31), leg bottom -0.74; hoof bottom ~ -0.84.
  part("leg_fl", capsule({ parent: "body", at: [-0.3, -0.5, -0.34], radius: 0.075, length: 0.48, material: "coat", joint: { pivot: [0, 0.24, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.3, -0.5, -0.34], radius: 0.075, length: 0.48, material: "coat", joint: { pivot: [0, 0.24, 0], axis: [1, 0, 0] } }));
  part("leg_bl", capsule({ parent: "body", at: [-0.3, -0.5, 0.36], radius: 0.08, length: 0.48, material: "coat", joint: { pivot: [0, 0.24, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.3, -0.5, 0.36], radius: 0.08, length: 0.48, material: "coat", joint: { pivot: [0, 0.24, 0], axis: [1, 0, 0] } }));

  // Small cloven hooves: two narrow toes split front-to-back.
  part("hoof_fl", box({ parent: "leg_fl", at: [0, -0.3, -0.04], size: [0.13, 0.08, 0.07], material: "hoof" }));
  part("hoof_fl_b", box({ parent: "leg_fl", at: [0, -0.3, 0.05], size: [0.13, 0.08, 0.07], material: "hoof" }));
  part("hoof_fr", box({ parent: "leg_fr", at: [0, -0.3, -0.04], size: [0.13, 0.08, 0.07], material: "hoof" }));
  part("hoof_fr_b", box({ parent: "leg_fr", at: [0, -0.3, 0.05], size: [0.13, 0.08, 0.07], material: "hoof" }));
  part("hoof_bl", box({ parent: "leg_bl", at: [0, -0.3, -0.04], size: [0.13, 0.08, 0.07], material: "hoof" }));
  part("hoof_bl_b", box({ parent: "leg_bl", at: [0, -0.3, 0.05], size: [0.13, 0.08, 0.07], material: "hoof" }));
  part("hoof_br", box({ parent: "leg_br", at: [0, -0.3, -0.04], size: [0.13, 0.08, 0.07], material: "hoof" }));
  part("hoof_br_b", box({ parent: "leg_br", at: [0, -0.3, 0.05], size: [0.13, 0.08, 0.07], material: "hoof" }));

  // Short upturned tail flicking up off the rump.
  part("tail", capsule({
    parent: "body",
    at: [0, 0.3, 0.58],
    rot: [-48, 0, 0],
    radius: 0.06,
    length: 0.3,
    material: "coat",
    joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
  }));
  part("tail_tip", sphere({ parent: "tail", at: [0, 0.17, 0], radius: 0.065, material: "coat_shadow" }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.0,
    cycleDistance: 0.72,
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
    bodyBob: 0.01,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.64,
    swingDegrees: 17,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      // Floppy ears lag a beat behind the body bob with a little overshoot.
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 13, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 13, overshoot: 0.6, lag: 0.12 }),
      // Beard swings gently under the chin, trailing the body.
      followThrough("beard", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.7, lag: 0.16 }),
    ],
  });
});
