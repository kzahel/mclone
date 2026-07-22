import { figure } from "../../src/dsl";

// A friendly saguaro whose natural branches read as arms, with short root
// feet, a restrained face, desert waddle, and temporary rain-bloom action.
export default figure("walking_cactus", ({
  asciiTexture,
  bipedWalk,
  box,
  clip,
  defaultClip,
  mat,
  metadata,
  part,
}) => {
  metadata({
    bodyPlans: ["biped", "rooted"],
    disposition: "neutral",
    groups: ["plant", "fantasy"],
    habitats: ["land"],
    scale: "medium",
    themes: ["cactus", "desert", "flowering", "living-growth", "mobile"],
  });

  mat("cactus", "#3f7d4e");
  mat("cactus_light", "#66a45d");
  mat("cactus_dark", "#285a3b");
  mat("root", "#6a5438");
  mat("spine", "#d6d0a3");
  mat("flower", "#d86186");
  mat("flower_light", "#f19bb1");
  mat("pollen", "#f1ce5d");

  asciiTexture("cactus_face", {
    palette: {
      ".": "#3f7d4e",
      "l": "#66a45d",
      "d": "#285a3b",
      "s": "#d6d0a3",
      "e": "#17261d",
    },
    pixels: [
      "s..l....l..s",
      "..l......l..",
      "....ee.ee...",
      "....ee.ee...",
      "s.....d....s",
      ".....dd.....",
      "..l......l..",
      "s..........s",
    ],
  });
  asciiTexture("cactus_ribs", {
    palette: { ".": "#3f7d4e", "l": "#66a45d", "d": "#285a3b", "s": "#d6d0a3" },
    pixels: ["s..l..s.", ".l....l.", "s..d..s.", ".l....l.", "s..l..s.", ".d....d."],
  });

  part("trunk", box({
    at: [0, 1.04, 0],
    size: [0.58, 1.28, 0.5],
    material: "cactus",
    faces: {
      north: { texture: "cactus_face" },
      east: { texture: "cactus_ribs" },
      west: { texture: "cactus_ribs" },
    },
  }));
  part("crown", box({
    parent: "trunk",
    at: [0, 0.68, 0],
    size: [0.48, 0.22, 0.44],
    material: "cactus_light",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`arm_${side}`, box({
      parent: "trunk",
      at: [sign * 0.43, 0.12, 0],
      rot: [0, 0, sign * -3],
      size: [0.56, 0.22, 0.24],
      material: "cactus",
      faces: { north: { texture: "cactus_ribs" }, south: { texture: "cactus_ribs" } },
      joint: { pivot: [sign * -0.24, 0, 0], axis: [1, 0, 0] },
    }));
    part(`arm_tip_${side}`, box({
      parent: `arm_${side}`,
      at: [sign * 0.23, 0.27, 0],
      rot: [0, 0, sign * 2],
      size: [0.22, 0.62, 0.22],
      material: "cactus_light",
      faces: { east: { texture: "cactus_ribs" }, west: { texture: "cactus_ribs" } },
    }));
    part(`arm_cap_${side}`, box({
      parent: `arm_tip_${side}`,
      at: [0, 0.34, 0],
      size: [0.2, 0.16, 0.2],
      material: "cactus",
    }));
    part(`leg_${side}`, box({
      parent: "trunk",
      at: [sign * 0.17, -0.75, 0.02],
      size: [0.25, 0.38, 0.28],
      material: "root",
      joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [sign * 0.03, -0.23, -0.08],
      size: [0.34, 0.12, 0.42],
      material: "root",
    }));
  }

  part("flower_center", box({
    parent: "crown",
    at: [0.03, 0.2, -0.02],
    rot: [0, 0, 45],
    size: [0.2, 0.2, 0.16],
    material: "pollen",
  }));
  for (const [name, x, y] of [
    ["top", 0, 0.17],
    ["bottom", 0, -0.17],
    ["l", -0.17, 0],
    ["r", 0.17, 0],
  ] as const) {
    const vertical = name === "top" || name === "bottom";
    part(`flower_petal_${name}`, box({
      parent: "flower_center",
      at: [x, y, 0],
      size: vertical ? [0.16, 0.24, 0.1] : [0.24, 0.16, 0.1],
      material: name === "top" || name === "l" ? "flower_light" : "flower",
    }));
  }

  bipedWalk("cactus_waddle", {
    label: "Cactus waddle",
    fps: 19,
    duration: 1.12,
    cycleDistance: 0.48,
    loop: true,
    samples: 23,
    armSwingDegrees: 5,
    body: "trunk",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    leftArm: "arm_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "arm_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.72,
    swingDegrees: 15,
  });
  clip("rain_bloom", {
    label: "Rain bloom",
    role: "action",
    nextClip: "cactus_waddle",
    fps: 30,
    loop: false,
    keys: [
      ["trunk", 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["trunk", 0.26, { at: [0, -0.03, 0], scale: [1.04, 0.96, 1.04] }],
      ["trunk", 0.56, { at: [0, 0.05, 0], scale: [0.98, 1.05, 0.98] }],
      ["trunk", 1.28, { at: [0, 0, 0], scale: [1, 1, 1] }],
      ["arm_l", 0, { rot: [0, 0, 0] }],
      ["arm_l", 0.56, { rot: [0, 0, 16] }],
      ["arm_l", 0.9, { rot: [0, 0, 10] }],
      ["arm_l", 1.28, { rot: [0, 0, 0] }],
      ["arm_r", 0, { rot: [0, 0, 0] }],
      ["arm_r", 0.56, { rot: [0, 0, -16] }],
      ["arm_r", 0.9, { rot: [0, 0, -10] }],
      ["arm_r", 1.28, { rot: [0, 0, 0] }],
      ["flower_center", 0, { rot: [0, 0, 0], scale: [1, 1, 1] }],
      ["flower_center", 0.26, { rot: [0, 0, -12], scale: [0.75, 0.75, 0.75] }],
      ["flower_center", 0.56, { rot: [0, 0, 8], scale: [1.38, 1.38, 1.18] }],
      ["flower_center", 0.9, { rot: [0, 0, -5], scale: [1.2, 1.2, 1.08] }],
      ["flower_center", 1.28, { rot: [0, 0, 0], scale: [1, 1, 1] }],
    ],
  });
  defaultClip("cactus_waddle");
});
