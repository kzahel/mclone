import { figure, type ClipKey } from "../../src/dsl";

// A connected colony of five lantern mushrooms. Individual caps pulse out of
// phase before the whole organism releases a smooth, softly glowing spore veil.
export default figure("lantern_mycelium", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  mat,
  metadata,
  part,
}) => {
  metadata({
    bodyPlans: ["rooted", "colony"],
    disposition: "neutral",
    groups: ["fungus"],
    habitats: ["land", "underground"],
    scale: "large",
    themes: ["bioluminescent", "colony", "living-growth", "mushroom", "nocturnal"],
  });

  mat("mycelium", { color: "#718d68", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("stem", "#c9c9a8");
  mat("stem_dark", "#8fa085");
  mat("cap_blue", "#496f72");
  mat("cap_teal", "#397b70");
  mat("cap_violet", "#675879");
  mat("gill_glow", { color: "#a8ffe0", alphaMode: "additive", opacity: 0.9 });
  mat("spore", { color: "#9cefd4", alphaMode: "blend", opacity: 0.3 });

  asciiTexture("mycelium_web", {
    palette: { ".": "transparent", "m": "#718d68", "l": "#a2b99a", "d": "#4a674f" },
    pixels: [
      "..m....m..",
      ".mmm..mmm.",
      "m.lmmmm.lm",
      "..mmmmmm..",
      "mmlmmmmmlm",
      ".mmm..mmm.",
      "..m....m..",
    ],
  });
  asciiTexture("cap_spots", {
    palette: { ".": "#496f72", "l": "#73a594", "g": "#a8ffe0", "d": "#304e58" },
    pixels: [
      "ddllllllllldd",
      "d..g....g...d",
      "..gg........l",
      "l......gg....",
      "...g...gg...l",
      "d..........dd",
    ],
  });
  asciiTexture("glowing_gills", {
    palette: { ".": "#a8ffe0", "l": "#dcfff2", "d": "#55bfa8" },
    pixels: [
      "llll..llll",
      "l..l..l..l",
      ".d.d..d.d.",
      "d..d..d..d",
      ".d.d..d.d.",
    ],
  });
  asciiTexture("spore_mist", {
    palette: {
      ".": "transparent",
      "f": "#9cefd42a",
      "m": "#9cefd45c",
      "b": "#c8ffe388",
    },
    pixels: [
      "..f.....",
      ".f..m.f.",
      "...b....",
      "f.m..f..",
      "..f...m.",
      ".m..b...",
      "...f..f.",
      ".....f..",
    ],
  });

  part("mycelium_mat", box({
    at: [0, 0.12, 0],
    size: [1.72, 0.18, 1.38],
    material: "mycelium",
    faces: {
      north: { texture: "mycelium_web" },
      south: { texture: "mycelium_web" },
      east: { texture: "mycelium_web" },
      west: { texture: "mycelium_web" },
      up: { texture: "mycelium_web" },
      down: { texture: "mycelium_web" },
    },
  }));
  part("web_l", box({
    parent: "mycelium_mat",
    at: [-0.72, 0.02, 0.18],
    rot: [0, -16, 0],
    size: [0.7, 0.08, 0.18],
    material: "mycelium",
    faces: { up: { texture: "mycelium_web" }, down: { texture: "mycelium_web" } },
  }));
  part("web_r", box({
    parent: "mycelium_mat",
    at: [0.7, 0.025, -0.2],
    rot: [0, 12, 0],
    size: [0.68, 0.075, 0.18],
    material: "mycelium",
    faces: { up: { texture: "mycelium_web" }, down: { texture: "mycelium_web" } },
  }));
  part("web_front", box({
    parent: "mycelium_mat",
    at: [0.12, 0.018, -0.62],
    rot: [0, -8, 0],
    size: [0.18, 0.07, 0.66],
    material: "mycelium",
    faces: { up: { texture: "mycelium_web" }, down: { texture: "mycelium_web" } },
  }));
  part("web_back", box({
    parent: "mycelium_mat",
    at: [-0.18, 0.015, 0.6],
    rot: [0, 10, 0],
    size: [0.18, 0.065, 0.62],
    material: "mycelium",
    faces: { up: { texture: "mycelium_web" }, down: { texture: "mycelium_web" } },
  }));

  const mushrooms = [
    { name: "center", x: 0, z: 0.04, height: 1.34, width: 1.12, depth: 0.96, material: "cap_blue", phase: 0 },
    { name: "left", x: -0.57, z: -0.24, height: 0.9, width: 0.76, depth: 0.68, material: "cap_teal", phase: 0.24 },
    { name: "right", x: 0.6, z: 0.2, height: 1.02, width: 0.82, depth: 0.72, material: "cap_violet", phase: 0.48 },
    { name: "front", x: 0.28, z: -0.48, height: 0.66, width: 0.62, depth: 0.56, material: "cap_teal", phase: 0.72 },
    { name: "back", x: -0.3, z: 0.48, height: 0.76, width: 0.68, depth: 0.6, material: "cap_blue", phase: 0.88 },
  ] as const;
  for (const mushroom of mushrooms) {
    part(`stem_${mushroom.name}`, box({
      parent: "mycelium_mat",
      at: [mushroom.x, mushroom.height / 2 - 0.03, mushroom.z],
      rot: [mushroom.z * -5, 0, mushroom.x * -5],
      size: [mushroom.width * 0.22, mushroom.height, mushroom.depth * 0.22],
      material: mushroom.name === "right" ? "stem_dark" : "stem",
      joint: { pivot: [0, -mushroom.height * 0.46, 0], axis: [0, 0, 1] },
    }));
    part(`cap_${mushroom.name}`, box({
      parent: `stem_${mushroom.name}`,
      at: [0, mushroom.height / 2 + 0.07, 0],
      rot: [0, mushroom.x * 5, mushroom.z * -4],
      size: [mushroom.width, 0.3, mushroom.depth],
      material: mushroom.material,
      faces: { up: { texture: "cap_spots" } },
    }));
    part(`cap_top_${mushroom.name}`, box({
      parent: `cap_${mushroom.name}`,
      at: [0, 0.19, 0],
      size: [mushroom.width * 0.68, 0.16, mushroom.depth * 0.68],
      material: mushroom.material,
      faces: { up: { texture: "cap_spots" } },
    }));
    part(`gills_${mushroom.name}`, box({
      parent: `cap_${mushroom.name}`,
      at: [0, -0.18, 0],
      size: [mushroom.width * 0.74, 0.09, mushroom.depth * 0.74],
      material: "gill_glow",
      faces: { down: { texture: "glowing_gills" } },
    }));
  }

  for (const [name, parent, at, size] of [
    ["center", "cap_center", [0, 0.16, 0], [0.7, 0.58, 0.66]],
    ["left", "cap_left", [-0.04, 0.12, -0.02], [0.48, 0.42, 0.46]],
    ["right", "cap_right", [0.05, 0.14, 0.03], [0.52, 0.46, 0.5]],
  ] as const) {
    part(`spore_cloud_${name}`, box({
      parent,
      at,
      size,
      material: "spore",
      faces: {
        north: { texture: "spore_mist" },
        south: { texture: "spore_mist" },
        east: { texture: "spore_mist" },
        west: { texture: "spore_mist" },
        up: { texture: "spore_mist" },
        down: { texture: "spore_mist" },
      },
    }));
  }

  const pulseKeys: ClipKey[] = [];
  for (const mushroom of mushrooms) {
    const direction = mushroom.x < 0 ? -1 : 1;
    pulseKeys.push(
      [`stem_${mushroom.name}`, 0, { rot: [0, 0, 0] }],
      [`stem_${mushroom.name}`, 0.6, { rot: [0, 0, direction * (2 + mushroom.phase * 2)] }],
      [`stem_${mushroom.name}`, 1.2, { rot: [0, 0, direction * -(2.5 + mushroom.phase * 2)] }],
      [`stem_${mushroom.name}`, 1.8, { rot: [0, 0, direction * 1.5] }],
      [`stem_${mushroom.name}`, 2.4, { rot: [0, 0, 0] }],
      [`cap_${mushroom.name}`, 0, { scale: [1, 1, 1] }],
      [`cap_${mushroom.name}`, 0.6, { scale: [1.02, 1.04 + mushroom.phase * 0.03, 1.02] }],
      [`cap_${mushroom.name}`, 1.2, { scale: [0.98, 0.96, 0.98] }],
      [`cap_${mushroom.name}`, 1.8, { scale: [1.03, 1.06, 1.03] }],
      [`cap_${mushroom.name}`, 2.4, { scale: [1, 1, 1] }],
    );
  }
  for (const cloud of ["center", "left", "right"] as const) {
    pulseKeys.push(
      [`spore_cloud_${cloud}`, 0, { scale: [0.14, 0.14, 0.14] }],
      [`spore_cloud_${cloud}`, 1.2, { scale: [0.2, 0.24, 0.2] }],
      [`spore_cloud_${cloud}`, 2.4, { scale: [0.14, 0.14, 0.14] }],
    );
  }
  clip("lantern_pulse", {
    label: "Lantern pulse",
    role: "idle",
    fps: 24,
    loop: true,
    keys: pulseKeys,
  });

  const bloomKeys: ClipKey[] = [];
  for (const mushroom of mushrooms) {
    const direction = mushroom.x < 0 ? -1 : 1;
    bloomKeys.push(
      [`stem_${mushroom.name}`, 0, { rot: [0, 0, 0] }],
      [`stem_${mushroom.name}`, 0.34, { rot: [0, 0, direction * 7] }],
      [`stem_${mushroom.name}`, 0.62, { rot: [0, 0, direction * -5] }],
      [`stem_${mushroom.name}`, 1.52, { rot: [0, 0, 0] }],
      [`cap_${mushroom.name}`, 0, { scale: [1, 1, 1] }],
      [`cap_${mushroom.name}`, 0.34, { scale: [0.9, 0.76, 0.9] }],
      [`cap_${mushroom.name}`, 0.62, { scale: [1.14, 1.22, 1.14] }],
      [`cap_${mushroom.name}`, 0.9, { scale: [1.05, 1.08, 1.05] }],
      [`cap_${mushroom.name}`, 1.52, { scale: [1, 1, 1] }],
    );
  }
  for (const cloud of ["center", "left", "right"] as const) {
    bloomKeys.push(
      [`spore_cloud_${cloud}`, 0, { at: [0, 0, 0], scale: [0.12, 0.12, 0.12] }],
      [`spore_cloud_${cloud}`, 0.34, { at: [0, 0.02, 0], scale: [0.18, 0.15, 0.18] }],
      [`spore_cloud_${cloud}`, 0.62, { at: [0, 0.22, 0], scale: [1.8, 1.5, 1.8] }],
      [`spore_cloud_${cloud}`, 0.9, { at: [0, 0.42, 0], scale: [2.4, 2.15, 2.4] }],
      [`spore_cloud_${cloud}`, 1.2, { at: [0, 0.54, 0], scale: [2.7, 2.45, 2.7] }],
      [`spore_cloud_${cloud}`, 1.52, { at: [0, 0, 0], scale: [0.12, 0.12, 0.12] }],
    );
  }
  clip("spore_bloom", {
    label: "Spore bloom",
    role: "action",
    nextClip: "lantern_pulse",
    fps: 30,
    loop: false,
    keys: bloomKeys,
  });
  defaultClip("lantern_pulse");
});
