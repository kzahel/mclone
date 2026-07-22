import { figure } from "../../src/dsl";

// A rooted harvest-field scarecrow with a broad crossbar silhouette, stitched
// sack face, binary-cutout hat and poncho, wind creak, and startle action.
export default figure("scarecrow", ({ asciiTexture, box, clip, defaultClip, mat, metadata, part }) => {
  metadata({
    bodyPlans: ["biped", "other"],
    disposition: "hostile",
    groups: ["fantasy", "monster", "humanoid", "construct"],
    habitats: ["land"],
    scale: "medium",
    themes: ["alpha-cutout", "harvest", "rural", "scary", "straw"],
  });
  mat("wood", "#60452d");
  mat("wood_dark", "#382b22");
  mat("straw", "#c8a655");
  mat("straw_light", "#e0c474");
  mat("burlap", "#8d7048");
  mat("burlap_dark", "#57462f");
  mat("cloth", { color: "#553d35", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("cloth_dark", { color: "#2f2928", alphaMode: "mask", alphaCutoff: 0.1 });

  asciiTexture("sack_face", {
    palette: { ".": "#8d7048", "l": "#b09261", "d": "#57462f", "s": "#282321" },
    pixels: [
      "ll......ll",
      "l........l",
      "..ss..ss..",
      ".s.s..s.s.",
      "....ss....",
      "..s....s..",
      ".s.ssss.s.",
      "dd......dd",
    ],
  });
  asciiTexture("ragged_poncho", {
    palette: { ".": "transparent", "c": "#553d35", "d": "#352b29", "s": "#c8a655" },
    pixels: [
      "..cccccccc..",
      ".cccccccccc.",
      "cccccccccccc",
      "cccdccccdccc",
      "ccccdcccdccc",
      "ccsccccccscc",
      "c.cccc.cccc.",
      ".cc.c..c.cc.",
    ],
  });
  asciiTexture("torn_brim", {
    palette: { ".": "transparent", "c": "#2f2928", "l": "#514441" },
    pixels: [
      "...cccccc...",
      ".cccccccccc.",
      "ccclccccclcc",
      "cccc..cccccc",
      ".ccccccccc..",
      "..ccc.ccc...",
    ],
  });

  part("stake", box({
    at: [0, 0.7, 0.12],
    size: [0.18, 1.4, 0.18],
    material: "wood_dark",
  }));
  part("torso", box({
    parent: "stake",
    at: [0, 0.4, -0.12],
    rot: [0, 0, -2],
    size: [0.68, 0.7, 0.36],
    material: "burlap",
    joint: { pivot: [0, -0.32, 0.08], axis: [1, 0, 0] },
  }));
  part("crossbar", box({
    parent: "torso",
    at: [0, 0.22, 0.04],
    rot: [0, 0, 2],
    size: [1.64, 0.16, 0.2],
    material: "wood",
  }));
  part("poncho", box({
    parent: "torso",
    at: [0, 0.02, -0.02],
    size: [1.2, 0.52, 0.48],
    material: "cloth",
    faces: {
      north: { texture: "ragged_poncho" },
      south: { texture: "ragged_poncho" },
      east: { texture: "ragged_poncho" },
      west: { texture: "ragged_poncho" },
      up: { texture: "ragged_poncho" },
      down: { texture: "ragged_poncho" },
    },
  }));
  part("coat_tail_l", box({
    parent: "poncho",
    at: [-0.25, -0.38, 0.02],
    rot: [5, 0, -6],
    size: [0.4, 0.38, 0.38],
    material: "cloth",
    faces: { north: { texture: "ragged_poncho" }, south: { texture: "ragged_poncho" } },
    joint: { pivot: [0, 0.17, 0.12], axis: [1, 0, 0] },
  }));
  part("coat_tail_r", box({
    parent: "poncho",
    at: [0.25, -0.35, 0],
    rot: [-4, 0, 7],
    size: [0.38, 0.34, 0.36],
    material: "cloth_dark",
    faces: { north: { texture: "ragged_poncho" }, south: { texture: "ragged_poncho" } },
    joint: { pivot: [0, 0.15, 0.12], axis: [1, 0, 0] },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`sleeve_${side}`, box({
      parent: "crossbar",
      at: [sign * 0.52, -0.05, -0.02],
      rot: [0, 0, sign * 4],
      size: [0.44, 0.3, 0.34],
      material: side === "l" ? "cloth" : "cloth_dark",
      faces: { north: { texture: "ragged_poncho" }, south: { texture: "ragged_poncho" } },
      joint: { pivot: [sign * -0.2, 0, 0], axis: [0, 0, 1] },
    }));
    part(`straw_arm_${side}`, box({
      parent: `sleeve_${side}`,
      at: [sign * 0.38, 0, 0],
      rot: [0, sign * 4, sign * -2],
      size: [0.34, 0.14, 0.16],
      material: "straw",
    }));
    part(`straw_hand_${side}`, box({
      parent: `straw_arm_${side}`,
      at: [sign * 0.25, 0, 0],
      rot: [0, sign * -7, sign * 4],
      size: [0.2, 0.22, 0.2],
      material: "straw_light",
    }));
  }

  part("neck_straw", box({
    parent: "torso",
    at: [0, 0.48, 0],
    size: [0.26, 0.28, 0.24],
    material: "straw",
    joint: { pivot: [0, -0.11, 0], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck_straw",
    at: [0.02, 0.28, -0.04],
    rot: [0, -4, 3],
    size: [0.64, 0.58, 0.5],
    material: "burlap",
    faces: { north: { texture: "sack_face" } },
  }));
  part("hat_brim", box({
    parent: "head",
    at: [-0.04, 0.33, 0],
    rot: [0, 0, -4],
    size: [1.08, 0.08, 0.68],
    material: "cloth_dark",
    faces: {
      north: { texture: "torn_brim" },
      south: { texture: "torn_brim" },
      east: { texture: "torn_brim" },
      west: { texture: "torn_brim" },
      up: { texture: "torn_brim" },
      down: { texture: "torn_brim" },
    },
  }));
  part("hat_crown", box({
    parent: "hat_brim",
    at: [0.05, 0.19, 0.02],
    rot: [0, 0, 5],
    size: [0.62, 0.32, 0.44],
    material: "cloth_dark",
  }));
  part("hat_band", box({
    parent: "hat_crown",
    at: [0, -0.02, -0.23],
    size: [0.58, 0.1, 0.08],
    material: "burlap_dark",
  }));
  part("hat_straw", box({
    parent: "hat_crown",
    at: [0.2, 0.2, 0.02],
    rot: [0, 0, -18],
    size: [0.16, 0.26, 0.16],
    material: "straw_light",
  }));

  clip("wind_creak", {
    label: "Wind creak",
    role: "idle",
    fps: 24,
    loop: true,
    keys: [
      ["torso", 0, { rot: [0, 0, 0] }],
      ["torso", 0.55, { rot: [1, 2, 3] }],
      ["torso", 1.1, { rot: [-1, -2, -3] }],
      ["torso", 1.65, { rot: [0, 0, 0] }],
      ["neck_straw", 0, { rot: [0, 0, 0] }],
      ["neck_straw", 0.55, { rot: [0, 7, -2] }],
      ["neck_straw", 1.1, { rot: [0, -6, 2] }],
      ["neck_straw", 1.65, { rot: [0, 0, 0] }],
      ["coat_tail_l", 0, { rot: [0, 0, 0] }],
      ["coat_tail_l", 0.55, { rot: [10, 0, 3] }],
      ["coat_tail_l", 1.1, { rot: [-7, 0, -2] }],
      ["coat_tail_l", 1.65, { rot: [0, 0, 0] }],
      ["coat_tail_r", 0, { rot: [0, 0, 0] }],
      ["coat_tail_r", 0.55, { rot: [-8, 0, -2] }],
      ["coat_tail_r", 1.1, { rot: [11, 0, 3] }],
      ["coat_tail_r", 1.65, { rot: [0, 0, 0] }],
    ],
  });
  clip("crow_startle", {
    label: "Crow startle",
    role: "action",
    nextClip: "wind_creak",
    fps: 30,
    loop: false,
    keys: [
      ["torso", 0, { rot: [0, 0, 0] }],
      ["torso", 0.2, { rot: [8, 0, 0] }],
      ["torso", 0.43, { rot: [-24, 0, 0] }],
      ["torso", 0.72, { rot: [-16, 0, 0] }],
      ["torso", 1.16, { rot: [0, 0, 0] }],
      ["neck_straw", 0, { rot: [0, 0, 0] }],
      ["neck_straw", 0.2, { rot: [0, -18, 0] }],
      ["neck_straw", 0.43, { rot: [0, 25, -8] }],
      ["neck_straw", 1.16, { rot: [0, 0, 0] }],
      ["sleeve_l", 0, { rot: [0, 0, 0] }],
      ["sleeve_l", 0.43, { rot: [0, 0, 44] }],
      ["sleeve_l", 0.72, { rot: [0, 0, 28] }],
      ["sleeve_l", 1.16, { rot: [0, 0, 0] }],
      ["sleeve_r", 0, { rot: [0, 0, 0] }],
      ["sleeve_r", 0.43, { rot: [0, 0, -44] }],
      ["sleeve_r", 0.72, { rot: [0, 0, -28] }],
      ["sleeve_r", 1.16, { rot: [0, 0, 0] }],
      ["coat_tail_l", 0, { rot: [0, 0, 0] }],
      ["coat_tail_l", 0.43, { rot: [24, 0, -6] }],
      ["coat_tail_l", 1.16, { rot: [0, 0, 0] }],
      ["coat_tail_r", 0, { rot: [0, 0, 0] }],
      ["coat_tail_r", 0.43, { rot: [20, 0, 7] }],
      ["coat_tail_r", 1.16, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("wind_creak");
});
