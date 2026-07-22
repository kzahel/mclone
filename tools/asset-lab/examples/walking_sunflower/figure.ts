import { figure } from "../../src/dsl";

// A sunflower made mobile by its own anatomy: a fibrous root crown divides
// into two walking roots, broad leaves act as balancing limbs, and eight fixed
// double-sided ray florets surround a seed-heavy flower head.
export default figure("walking_sunflower", ({
  asciiTexture,
  bipedWalk,
  box,
  clip,
  defaultClip,
  mat,
  metadata,
  part,
  plane,
  swing,
}) => {
  metadata({
    bodyPlans: ["biped", "rooted"],
    disposition: "passive",
    groups: ["plant", "fantasy"],
    habitats: ["land"],
    scale: "medium",
    themes: ["flowering", "living-growth", "mobile", "sunflower"],
  });

  mat("root", "#65523a");
  mat("root_light", "#8a7148");
  mat("stem", "#4f7d3e");
  mat("stem_light", "#72a554");
  mat("leaf", "#376637");
  mat("leaf_light", "#5f9145");
  mat("seed", "#493021");
  mat("seed_light", "#806039");
  mat("petal_gold", { color: "#e4ac32", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("petal_light", { color: "#f2c94d", alphaMode: "mask", alphaCutoff: 0.1 });

  asciiTexture("root_rings", {
    palette: { ".": "#65523a", "l": "#8a7148", "d": "#403729" },
    pixels: ["dddddddd", "d.ll.l.d", ".l....l.", "..d..d..", "dddddddd"],
  });
  asciiTexture("stem_ribs", {
    palette: { ".": "#4f7d3e", "l": "#72a554", "d": "#376637" },
    pixels: ["l..d..l.", ".l....l.", "l..d..l.", ".l....l.", "l..d..l.", ".l....l."],
  });
  asciiTexture("seed_spiral", {
    palette: { ".": "#493021", "l": "#806039", "g": "#ad813c", "d": "#2e211b" },
    pixels: [
      "ddlllllllldd",
      "dllgggggglld",
      "dlggllllggld",
      "lgglddddlggl",
      "lglldggdllgl",
      "lgldgllgdggl",
      "lgldgllgdggl",
      "lglldggdllgl",
      "lgglddddlggl",
      "dlggllllggld",
      "dllgggggglld",
      "ddlllllllldd",
    ],
  });
  asciiTexture("ray_floret", {
    palette: { ".": "transparent", "y": "#e4ac32", "l": "#f2c94d", "d": "#bd7e20" },
    pixels: [
      "...yy...",
      "..ylly..",
      ".ylllly.",
      "yyllllyy",
      ".yyllyy.",
      "..yyyy..",
      "...yy...",
      "...yy...",
    ],
  });

  part("root_crown", box({
    at: [0, 0.52, 0],
    size: [0.52, 0.38, 0.48],
    material: "root",
    faces: { east: { texture: "root_rings" }, west: { texture: "root_rings" } },
  }));
  part("stem", box({
    parent: "root_crown",
    at: [0, 0.66, 0],
    size: [0.32, 1.32, 0.32],
    material: "stem",
    faces: { east: { texture: "stem_ribs" }, west: { texture: "stem_ribs" } },
    joint: { pivot: [0, -0.58, 0], axis: [0, 0, 1] },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`leaf_${side}`, box({
      parent: "stem",
      at: [sign * 0.38, -0.08, 0],
      rot: [0, sign * -5, sign * 18],
      size: [0.64, 0.22, 0.3],
      material: side === "l" ? "leaf" : "leaf_light",
      faces: { north: { texture: "stem_ribs" }, south: { texture: "stem_ribs" } },
      joint: { pivot: [sign * -0.27, 0, 0], axis: [1, 0, 0] },
    }));
    part(`root_leg_${side}`, box({
      parent: "root_crown",
      at: [sign * 0.15, -0.31, 0],
      size: [0.2, 0.38, 0.22],
      material: "root_light",
      joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
    }));
    part(`root_foot_${side}`, box({
      parent: `root_leg_${side}`,
      at: [sign * 0.03, -0.16, -0.08],
      size: [0.34, 0.1, 0.4],
      material: "root",
      faces: { up: { texture: "root_rings" } },
    }));
  }

  part("flower_core", box({
    parent: "stem",
    at: [0, 0.79, -0.02],
    size: [0.62, 0.62, 0.24],
    material: "seed",
    faces: { north: { texture: "seed_spiral" }, south: { texture: "seed_spiral" } },
    joint: { pivot: [0, -0.25, 0.08], axis: [0, 1, 0] },
  }));
  part("petal_collar", box({
    parent: "flower_core",
    at: [0, 0.25, -0.22],
    size: [0.46, 0.46, 0.08],
    material: "seed",
  }));

  const petals = [
    { name: "top", at: [0, 0.68, -0.24], rot: 0 },
    { name: "upper_r", at: [0.304, 0.554, -0.22], rot: -45 },
    { name: "r", at: [0.43, 0.25, -0.24], rot: -90 },
    { name: "lower_r", at: [0.304, -0.054, -0.22], rot: -135 },
    { name: "bottom", at: [0, -0.18, -0.24], rot: 180 },
    { name: "lower_l", at: [-0.304, -0.054, -0.22], rot: 135 },
    { name: "l", at: [-0.43, 0.25, -0.24], rot: 90 },
    { name: "upper_l", at: [-0.304, 0.554, -0.22], rot: 45 },
  ] as const;
  for (const [index, petal] of petals.entries()) {
    part(`petal_${petal.name}`, plane({
      parent: "flower_core",
      at: petal.at,
      rot: [0, 0, petal.rot],
      size: [0.34, 0.5],
      sidedness: "double",
      material: index % 2 === 0 ? "petal_light" : "petal_gold",
      texture: "ray_floret",
      joint: { pivot: [0, -0.21, 0], axis: [0, 0, 1] },
    }));
  }

  bipedWalk("root_step", {
    label: "Root step",
    fps: 20,
    duration: 1.16,
    cycleDistance: 0.42,
    loop: true,
    samples: 23,
    armSwingDegrees: 7,
    body: "root_crown",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    leftArm: "leaf_l",
    leftContact: "root_foot_l",
    leftLeg: "root_leg_l",
    rightArm: "leaf_r",
    rightContact: "root_foot_r",
    rightLeg: "root_leg_r",
    stanceRatio: 0.72,
    swingDegrees: 15,
    tracks: [
      swing("stem", { axis: "z", degrees: 1.8, phase: 0.25 }),
      swing("petal_top", { axis: "z", degrees: 2.4, phase: 0.12 }),
      swing("petal_r", { axis: "z", degrees: 2, phase: 0.37 }),
      swing("petal_l", { axis: "z", degrees: -2, phase: 0.63 }),
    ],
  });
  clip("sun_follow", {
    label: "Follow the sun",
    role: "idle",
    fps: 24,
    loop: true,
    keys: [
      ["stem", 0, { rot: [0, 0, -2] }],
      ["stem", 1.1, { rot: [0, 0, 2] }],
      ["stem", 2.2, { rot: [0, 0, -2] }],
      ["flower_core", 0, { rot: [0, -13, -2] }],
      ["flower_core", 1.1, { rot: [0, 15, 2] }],
      ["flower_core", 2.2, { rot: [0, -13, -2] }],
      ["leaf_l", 0, { rot: [0, 0, -3] }],
      ["leaf_l", 1.1, { rot: [0, 0, 7] }],
      ["leaf_l", 2.2, { rot: [0, 0, -3] }],
      ["leaf_r", 0, { rot: [0, 0, 3] }],
      ["leaf_r", 1.1, { rot: [0, 0, -7] }],
      ["leaf_r", 2.2, { rot: [0, 0, 3] }],
      ["petal_top", 0, { rot: [0, 0, -2] }],
      ["petal_top", 1.1, { rot: [0, 0, 3] }],
      ["petal_top", 2.2, { rot: [0, 0, -2] }],
      ["petal_bottom", 0, { rot: [0, 0, 2] }],
      ["petal_bottom", 1.1, { rot: [0, 0, -3] }],
      ["petal_bottom", 2.2, { rot: [0, 0, 2] }],
    ],
  });
  defaultClip("root_step");
});
