import { figure } from "../../src/dsl";

// A cave-dwelling ambush animal authored upright with a four-pad walk. Gameplay
// can roll the complete actor onto a ceiling without inverting its semantic rig.
export default figure("ceiling_angler", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  mat,
  metadata,
  part,
  quadrupedWalk,
  swing,
}) => {
  metadata({
    bodyPlans: ["crawler", "quadruped"],
    disposition: "hostile",
    groups: ["animal", "fantasy", "monster"],
    habitats: ["underground"],
    scale: "medium",
    themes: ["ambush", "cave", "nocturnal", "scary"],
  });

  mat("hide", "#394650");
  mat("hide_light", "#52646b");
  mat("hide_dark", "#202b33");
  mat("belly", "#67736f");
  mat("claw", "#171d21");
  mat("mouth", "#321b25");
  mat("gum", "#814052");
  mat("tooth", "#e8dfbc");
  mat("eye", "#e1a74b");
  mat("lure", { color: "#79e1b8", alphaMode: "additive", opacity: 0.9 });

  asciiTexture("cave_mottle", {
    palette: {
      ".": "#394650",
      "l": "#52646b",
      "d": "#202b33",
      "b": "#67736f",
    },
    pixels: [
      "dddddddddddd",
      "d..ll....l.d",
      "d.l..dd...ld",
      "d...bbbb...d",
      "dl...bb..l.d",
      "d..dd..ll..d",
      "dddddddddddd",
    ],
  });
  asciiTexture("angler_face", {
    palette: {
      ".": "#394650",
      "d": "#202b33",
      "e": "#e1a74b",
      "m": "#321b25",
    },
    pixels: [
      "dd........dd",
      "d.ee....ee.d",
      "..ee....ee..",
      "....dddd....",
      "..mmmmmmmm..",
      ".mm......mm.",
      "mm........mm",
    ],
  });
  asciiTexture("lure_mark", {
    palette: { ".": "#79e1b8", "l": "#c8ffe8", "d": "#2f8b76" },
    pixels: ["..ll..", ".llll.", "llddll", "llddll", ".llll.", "..ll.."],
  });

  part("body", box({
    at: [0, 1.28, 0],
    size: [0.96, 0.68, 0.64],
    material: "hide",
    faces: { east: { texture: "cave_mottle" }, west: { texture: "cave_mottle" } },
  }));
  part("abdomen", box({
    parent: "body",
    at: [0, 0.14, 0.36],
    size: [0.72, 0.56, 0.5],
    material: "hide_dark",
  }));
  part("belly_plate", box({
    parent: "body",
    at: [0, -0.34, 0.02],
    size: [0.62, 0.16, 0.46],
    material: "belly",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.39, -0.12],
    size: [0.52, 0.34, 0.46],
    material: "hide_dark",
    joint: { pivot: [0, -0.14, 0.12], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.3, -0.13],
    size: [0.84, 0.56, 0.66],
    material: "hide",
    faces: { north: { texture: "angler_face" } },
  }));
  part("upper_muzzle", box({
    parent: "head",
    at: [0, -0.05, -0.38],
    size: [0.68, 0.24, 0.22],
    material: "hide_dark",
    faces: { down: { material: "mouth" } },
  }));
  part("lower_jaw", box({
    parent: "head",
    at: [0, -0.27, -0.22],
    size: [0.64, 0.2, 0.5],
    material: "hide_light",
    faces: { up: { material: "mouth" } },
    joint: { pivot: [0, 0.08, 0.2], axis: [1, 0, 0] },
  }));
  part("lower_gum", box({
    parent: "lower_jaw",
    at: [0, 0.1, -0.16],
    size: [0.52, 0.08, 0.16],
    material: "gum",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_${side}`, box({
      parent: "head",
      at: [sign * 0.25, 0.08, -0.36],
      size: [0.14, 0.12, 0.08],
      material: "eye",
      faces: { north: { material: "claw" } },
    }));
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.44, 0.1, -0.02],
      rot: [0, sign * -8, sign * -28],
      size: [0.32, 0.34, 0.12],
      material: "hide_light",
    }));
  }

  for (const [index, x] of [-0.23, -0.08, 0.08, 0.23].entries()) {
    part(`upper_tooth_${index + 1}`, box({
      parent: "upper_muzzle",
      at: [x, -0.16, -0.04],
      rot: [0, 0, index % 2 === 0 ? -4 : 4],
      size: [0.075, index % 2 === 0 ? 0.18 : 0.14, 0.09],
      material: "tooth",
    }));
    part(`lower_tooth_${index + 1}`, box({
      parent: "lower_jaw",
      at: [x, 0.15, -0.16],
      rot: [0, 0, index % 2 === 0 ? 4 : -4],
      size: [0.07, index % 2 === 0 ? 0.15 : 0.12, 0.085],
      material: "tooth",
    }));
  }

  part("lure_root", box({
    parent: "head",
    at: [0, 0.39, -0.25],
    rot: [-8, 0, 0],
    size: [0.08, 0.34, 0.08],
    material: "hide_light",
    joint: { pivot: [0, -0.14, 0], axis: [0, 0, 1] },
  }));
  part("lure_drop", box({
    parent: "lure_root",
    at: [0, 0.28, -0.08],
    rot: [-12, 0, 0],
    size: [0.065, 0.32, 0.065],
    material: "belly",
    joint: { pivot: [0, -0.14, 0], axis: [0, 0, 1] },
  }));
  part("lure_bulb", box({
    parent: "lure_drop",
    at: [0, 0.21, -0.05],
    rot: [0, 0, 45],
    size: [0.23, 0.23, 0.2],
    material: "lure",
    faces: { north: { texture: "lure_mark" }, south: { texture: "lure_mark" } },
  }));

  for (const [row, z, x] of [
    ["front", -0.18, 0.47],
    ["rear", 0.25, 0.38],
  ] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`upper_limb_${row}_${side}`, box({
        parent: "body",
        at: [sign * x, -0.47, z],
        rot: [row === "front" ? 4 : -5, sign * 4, sign * 24],
        size: [0.18, 0.8, 0.18],
        material: row === "front" ? "hide_light" : "hide",
        joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] },
      }));
      part(`fore_limb_${row}_${side}`, box({
        parent: `upper_limb_${row}_${side}`,
        at: [sign * 0.12, -0.5, 0],
        rot: [0, 0, sign * -18],
        size: [0.16, 0.5, 0.16],
        material: "hide_dark",
        joint: { pivot: [0, 0.22, 0], axis: [0, 0, 1] },
      }));
      part(`ceiling_hook_${row}_${side}`, box({
        parent: `fore_limb_${row}_${side}`,
        at: [sign * 0.05, -0.29, 0],
        size: [0.34, 0.14, 0.3],
        material: "claw",
      }));
    }
  }

  quadrupedWalk("ceiling_walk", {
    label: "Ceiling walk",
    fps: 20,
    duration: 1.16,
    cycleDistance: 0.54,
    gait: "walk",
    loop: true,
    samples: 25,
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    contactParts: {
      frontLeft: "ceiling_hook_front_l",
      frontRight: "ceiling_hook_front_r",
      backLeft: "ceiling_hook_rear_l",
      backRight: "ceiling_hook_rear_r",
    },
    head: "neck",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "upper_limb_front_l",
      frontRight: "upper_limb_front_r",
      backLeft: "upper_limb_rear_l",
      backRight: "upper_limb_rear_r",
    },
    stanceRatio: 0.72,
    swingDegrees: 16,
    tracks: [
      swing("lure_root", { axis: "z", degrees: 7, phase: 0.08 }),
      swing("lure_drop", { axis: "z", degrees: 10, phase: 0.22 }),
    ],
  });
  clip("lure_strike", {
    label: "Lure strike",
    role: "action",
    nextClip: "ceiling_walk",
    fps: 30,
    loop: false,
    keys: [
      ["neck", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["neck", 0.2, { at: [0, -0.06, 0.04], rot: [-8, 0, 0] }],
      ["neck", 0.48, { at: [0, 0.24, -0.17], rot: [16, 0, 0] }],
      ["neck", 0.72, { at: [0, 0.19, -0.12], rot: [10, 0, 0] }],
      ["neck", 1.24, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["lower_jaw", 0, { rot: [0, 0, 0] }],
      ["lower_jaw", 0.2, { rot: [4, 0, 0] }],
      ["lower_jaw", 0.48, { rot: [-34, 0, 0] }],
      ["lower_jaw", 0.72, { rot: [-24, 0, 0] }],
      ["lower_jaw", 0.86, { rot: [2, 0, 0] }],
      ["lower_jaw", 1.24, { rot: [0, 0, 0] }],
      ["lure_drop", 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["lure_drop", 0.2, { at: [0, -0.06, 0], scale: [1, 0.72, 1] }],
      ["lure_drop", 0.48, { at: [0, 0.14, -0.05], scale: [1, 1.45, 1] }],
      ["lure_drop", 0.72, { at: [0, 0.1, -0.03], scale: [1, 1.25, 1] }],
      ["lure_drop", 1.24, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["upper_limb_front_l", 0, { rot: [0, 0, 0] }],
      ["upper_limb_front_l", 0.48, { rot: [0, 0, -7] }],
      ["upper_limb_front_l", 1.24, { rot: [0, 0, 0] }],
      ["upper_limb_front_r", 0, { rot: [0, 0, 0] }],
      ["upper_limb_front_r", 0.48, { rot: [0, 0, 7] }],
      ["upper_limb_front_r", 1.24, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("ceiling_walk");
});
