import { figure } from "../../src/dsl";

// A low grave-born crawler with a plated ribcage, split skull, four dragging
// limbs, a grounded scuttle, and a sudden maw-burst action.
export default figure("grave_crawler", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
  quadrupedWalk,
}) => {
  metadata({
    bodyPlans: ["crawler"],
    disposition: "hostile",
    groups: ["fantasy", "monster"],
    habitats: ["land", "underground"],
    scale: "medium",
    themes: ["burial", "graveyard", "horror", "scary", "undead"],
  });

  mat("bone", "#b8b197");
  mat("bone_light", "#d9d1b3");
  mat("bone_dark", "#777362");
  mat("flesh", "#544b49");
  mat("flesh_dark", "#312d30");
  mat("mouth", "#4d252d");
  mat("eye", "#b8df59");
  mat("claw", "#d6c89e");
  mat("soil", "#594536");

  asciiTexture("skull_face", {
    palette: {
      ".": "#b8b197",
      "l": "#d9d1b3",
      "d": "#777362",
      "e": "#b8df59",
      "v": "#26242a",
    },
    pixels: [
      "ddlllllllldd",
      "d..........d",
      ".ee......ee.",
      ".ev......ve.",
      "...dd..dd...",
      "....d..d....",
      "..vv....vv..",
      "dd........dd",
    ],
  });
  asciiTexture("jaw_teeth", {
    palette: { ".": "#4d252d", "t": "#d9d1b3", "d": "#312d30" },
    pixels: [
      "tt.tt..tt.tt",
      "t..t....t..t",
      "..dddddddd..",
      ".d........d.",
    ],
  });
  asciiTexture("rib_plate", {
    palette: { ".": "#544b49", "b": "#b8b197", "l": "#d9d1b3", "d": "#777362" },
    pixels: [
      "ddlllllllldd",
      "d.bb....bb.d",
      ".bb......bb.",
      "bb........bb",
      ".bb......bb.",
      "d.bb....bb.d",
    ],
  });
  asciiTexture("grave_claws", {
    palette: { ".": "#312d30", "c": "#d6c89e", "s": "#594536" },
    pixels: [
      "..........",
      ".c..c..c..",
      "ccsccscccc",
    ],
  });

  part("body", box({
    at: [0, 0.67, 0.08],
    size: [0.68, 0.34, 1.08],
    material: "flesh",
    faces: {
      up: { texture: "rib_plate" },
      east: { texture: "rib_plate" },
      west: { texture: "rib_plate" },
    },
  }));
  part("chest_plate", box({
    parent: "body",
    at: [0, 0.22, -0.24],
    size: [0.82, 0.16, 0.4],
    material: "bone",
    faces: { up: { texture: "rib_plate" } },
  }));
  part("middle_plate", box({
    parent: "body",
    at: [0, 0.2, 0.12],
    size: [0.72, 0.14, 0.28],
    material: "bone_dark",
    faces: { up: { texture: "rib_plate" } },
  }));
  part("rear_plate", box({
    parent: "body",
    at: [0, 0.18, 0.42],
    size: [0.6, 0.12, 0.26],
    material: "bone",
    faces: { up: { texture: "rib_plate" } },
  }));
  part("spine_knob", box({
    parent: "middle_plate",
    at: [0, 0.13, 0],
    rot: [0, 0, 45],
    size: [0.2, 0.2, 0.18],
    material: "bone_light",
  }));

  part("neck", box({
    parent: "body",
    at: [0, 0.03, -0.63],
    rot: [0, 0, 0],
    size: [0.36, 0.26, 0.32],
    material: "flesh_dark",
    joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.04, -0.3],
    size: [0.64, 0.42, 0.46],
    material: "bone",
    faces: { north: { texture: "skull_face" } },
  }));
  part("brow", box({
    parent: "head",
    at: [0, 0.14, -0.27],
    size: [0.52, 0.12, 0.12],
    material: "bone_dark",
  }));
  part("jaw", box({
    parent: "head",
    at: [0, -0.27, -0.04],
    rot: [-4, 0, 0],
    size: [0.58, 0.16, 0.4],
    material: "mouth",
    faces: { north: { texture: "jaw_teeth" } },
    joint: { pivot: [0, 0.07, 0.16], axis: [1, 0, 0] },
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.38, -0.3],
    ["fr", 0.38, -0.3],
    ["bl", -0.36, 0.34],
    ["br", 0.36, 0.34],
  ] as const) {
    const sign = x < 0 ? -1 : 1;
    part(`upper_leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.18, z],
      rot: [-5, 0, sign * -12],
      size: [0.24, 0.34, 0.24],
      material: suffix.startsWith("f") ? "bone_dark" : "flesh_dark",
      joint: { pivot: [0, 0.15, 0], axis: [1, 0, 0] },
    }));
    part(`foreleg_${suffix}`, box({
      parent: `upper_leg_${suffix}`,
      at: [sign * 0.1, -0.23, -0.02],
      rot: [8, 0, sign * -18],
      size: [0.2, 0.24, 0.2],
      material: "flesh_dark",
    }));
    part(`claw_${suffix}`, box({
      parent: `foreleg_${suffix}`,
      at: [sign * 0.07, -0.16, -0.08],
      rot: [0, sign * -5, 0],
      size: [0.32, 0.1, 0.38],
      material: "flesh_dark",
      faces: { north: { texture: "grave_claws" } },
    }));
  }

  part("tail_root", box({
    parent: "body",
    at: [0, 0.04, 0.64],
    rot: [-38, 0, 0],
    size: [0.3, 0.5, 0.3],
    material: "flesh_dark",
    joint: { pivot: [0, -0.22, 0], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_root",
    at: [0, 0.36, 0.02],
    rot: [-18, 0, 0],
    size: [0.22, 0.34, 0.22],
    material: "bone_dark",
  }));

  quadrupedWalk("grave_scuttle", {
    label: "Grave scuttle",
    fps: 22,
    duration: 0.78,
    cycleDistance: 0.58,
    gait: "trot",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "claw_fl",
      frontRight: "claw_fr",
      backLeft: "claw_bl",
      backRight: "claw_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "neck",
    headSwingDegrees: 5,
    legs: {
      frontLeft: "upper_leg_fl",
      frontRight: "upper_leg_fr",
      backLeft: "upper_leg_bl",
      backRight: "upper_leg_br",
    },
    stanceRatio: 0.62,
    swingDegrees: 19,
    tail: "tail_root",
    tailSwingDegrees: 9,
    tracks: [
      followThrough("jaw", {
        source: "body",
        sourceChannel: "pos",
        sourceAxis: "y",
        axis: "x",
        degrees: 5,
        overshoot: 0.46,
        lag: 0.1,
      }),
    ],
  });
  clip("maw_burst", {
    label: "Maw burst",
    role: "action",
    nextClip: "grave_scuttle",
    fps: 30,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["body", 0.24, { at: [0, 0.025, 0.08], rot: [4, 0, 0] }],
      ["body", 0.42, { at: [0, 0.04, -0.12], rot: [-7, 0, 0] }],
      ["body", 0.68, { at: [0, 0.025, -0.07], rot: [-4, 0, 0] }],
      ["body", 1.06, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.24, { rot: [0, -8, 0] }],
      ["neck", 0.42, { rot: [-11, 14, 0] }],
      ["neck", 0.68, { rot: [-7, -10, 0] }],
      ["neck", 1.06, { rot: [0, 0, 0] }],
      ["jaw", 0, { rot: [0, 0, 0] }],
      ["jaw", 0.24, { rot: [8, 0, 0] }],
      ["jaw", 0.42, { rot: [-43, 0, 0] }],
      ["jaw", 0.68, { rot: [-28, 0, 0] }],
      ["jaw", 1.06, { rot: [0, 0, 0] }],
      ["upper_leg_fl", 0, { rot: [0, 0, 0] }],
      ["upper_leg_fl", 0.42, { rot: [-14, 0, 5] }],
      ["upper_leg_fl", 1.06, { rot: [0, 0, 0] }],
      ["upper_leg_fr", 0, { rot: [0, 0, 0] }],
      ["upper_leg_fr", 0.42, { rot: [-14, 0, -5] }],
      ["upper_leg_fr", 1.06, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("grave_scuttle");
});
