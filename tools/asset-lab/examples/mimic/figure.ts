import { figure } from "../../src/dsl";

// A treasure-chest mimic that keeps a readable closed silhouette, breathes
// through its lid, and reveals teeth, eyes, and tongue during a snap attack.
export default figure("mimic", ({ asciiTexture, box, clip, defaultClip, mat, metadata, part }) => {
  metadata({
    bodyPlans: ["other"],
    disposition: "hostile",
    groups: ["fantasy", "monster"],
    habitats: ["land", "underground"],
    scale: "medium",
    themes: ["ambush", "dungeon", "scary", "treasure"],
  });
  mat("wood", "#6f4228");
  mat("wood_light", "#9a6337");
  mat("wood_dark", "#3e291f");
  mat("iron", "#34383a");
  mat("iron_light", "#666c6c");
  mat("mouth", "#25151a");
  mat("gum", "#7e333b");
  mat("tooth", "#e7ddb4");
  mat("tongue", "#a94350");
  mat("eye", "#dfb83d");

  asciiTexture("chest_front", {
    palette: { ".": "#6f4228", "l": "#9a6337", "d": "#3e291f", "i": "#34383a" },
    pixels: [
      "llllllllllll",
      "l..........l",
      "..dddddddd..",
      "..d......d..",
      "..d..ii..d..",
      "..d..ii..d..",
      "..dddddddd..",
      "dd........dd",
    ],
  });
  asciiTexture("lid_grain", {
    palette: { ".": "#6f4228", "l": "#9a6337", "d": "#3e291f" },
    pixels: [
      "llllllllllll",
      "ll........ll",
      "..dddddddd..",
      "............",
      "d..d....d..d",
      "dddddddddddd",
    ],
  });

  part("base", box({
    at: [0, 0.3, 0],
    size: [1.2, 0.5, 0.8],
    material: "wood",
    faces: { north: { texture: "chest_front" }, south: { texture: "chest_front" } },
  }));
  part("base_band", box({
    parent: "base",
    at: [0, -0.02, -0.43],
    size: [1.06, 0.14, 0.12],
    material: "iron",
  }));
  part("mouth_cavity", box({
    parent: "base",
    at: [0, 0.32, -0.28],
    size: [0.88, 0.3, 0.12],
    material: "mouth",
  }));
  part("gum", box({
    parent: "mouth_cavity",
    at: [0, -0.09, -0.02],
    size: [0.72, 0.1, 0.1],
    material: "gum",
  }));
  for (const [index, x] of [-0.28, -0.09, 0.1, 0.29].entries()) {
    part(`lower_tooth_${index + 1}`, box({
      parent: "mouth_cavity",
      at: [x, 0.08, -0.02],
      rot: [0, 0, index % 2 === 0 ? -5 : 5],
      size: [0.12, index % 2 === 0 ? 0.2 : 0.15, 0.12],
      material: "tooth",
    }));
  }
  part("eye_l", box({
    parent: "mouth_cavity",
    at: [-0.25, 0.14, -0.02],
    size: [0.13, 0.1, 0.1],
    material: "eye",
  }));
  part("eye_r", box({
    parent: "mouth_cavity",
    at: [0.25, 0.14, -0.02],
    size: [0.13, 0.1, 0.1],
    material: "eye",
  }));
  part("tongue", box({
    parent: "base",
    at: [0.08, 0.16, -0.05],
    rot: [4, 0, -5],
    size: [0.34, 0.12, 0.5],
    material: "tongue",
    joint: { pivot: [0, 0, 0.22], axis: [0, 1, 0] },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`hinge_${side}`, box({
      parent: "base",
      at: [sign * 0.48, 0.31, 0.29],
      size: [0.16, 0.18, 0.16],
      material: "iron_light",
    }));
    for (const [row, z] of [["front", -0.24], ["rear", 0.24]] as const) {
      part(`claw_${side}_${row}`, box({
        parent: "base",
        at: [sign * 0.43, -0.25, z],
        rot: [0, sign * (row === "front" ? -8 : 8), sign * 3],
        size: [0.22, 0.12, 0.3],
        material: "wood_dark",
      }));
    }
  }

  part("lid", box({
    parent: "base",
    at: [0, 0.4, 0.02],
    size: [1.18, 0.3, 0.78],
    material: "wood_light",
    faces: { north: { texture: "lid_grain" }, south: { texture: "lid_grain" } },
    joint: { pivot: [0, -0.14, 0.34], axis: [1, 0, 0] },
  }));
  part("lid_band", box({
    parent: "lid",
    at: [0, 0.02, -0.41],
    size: [0.18, 0.32, 0.12],
    material: "iron",
  }));
  part("latch", box({
    parent: "lid_band",
    at: [0, -0.18, -0.07],
    size: [0.3, 0.22, 0.12],
    material: "iron_light",
  }));
  for (const [index, x] of [-0.3, -0.1, 0.1, 0.3].entries()) {
    part(`upper_tooth_${index + 1}`, box({
      parent: "lid",
      at: [x, -0.1, -0.29],
      rot: [0, 0, index % 2 === 0 ? 5 : -5],
      size: [0.13, index % 2 === 0 ? 0.2 : 0.16, 0.14],
      material: "tooth",
    }));
  }

  clip("patient_breath", {
    label: "Patient breath",
    role: "idle",
    fps: 24,
    loop: true,
    keys: [
      ["lid", 0, { rot: [0, 0, 0] }],
      ["lid", 0.38, { rot: [2, 0, 0] }],
      ["lid", 0.76, { rot: [0.5, 0, 0] }],
      ["lid", 1.14, { rot: [3, 0, 0] }],
      ["lid", 1.52, { rot: [0, 0, 0] }],
      ["tongue", 0, { rot: [0, 0, 0] }],
      ["tongue", 0.38, { rot: [0, 5, 4] }],
      ["tongue", 0.76, { rot: [0, -4, -3] }],
      ["tongue", 1.14, { rot: [0, 6, 3] }],
      ["tongue", 1.52, { rot: [0, 0, 0] }],
    ],
  });
  clip("snap_attack", {
    label: "Snap attack",
    role: "action",
    nextClip: "patient_breath",
    fps: 30,
    loop: false,
    keys: [
      ["lid", 0, { rot: [0, 0, 0] }],
      ["lid", 0.18, { rot: [18, 0, 0] }],
      ["lid", 0.34, { rot: [72, 0, 0] }],
      ["lid", 0.52, { rot: [16, 0, 0] }],
      ["lid", 0.68, { rot: [66, 0, 0] }],
      ["lid", 1.08, { rot: [0, 0, 0] }],
      ["tongue", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["tongue", 0.34, { at: [0, 0, -0.72], rot: [0, 14, 8] }],
      ["tongue", 0.68, { at: [0, 0, -0.58], rot: [0, -12, -6] }],
      ["tongue", 1.08, { at: [0, 0, 0], rot: [0, 0, 0] }],
    ],
  });
  defaultClip("patient_breath");
});
