import type { FigureApi } from "../src/dsl";

export type GhostRenderingStyle = "dither" | "blend";

// Shared geometry and motion keep the two Ghosts as a rendering comparison,
// not two independently modeled characters.
export function buildGhost(api: FigureApi, style: GhostRenderingStyle): void {
  const {
    asciiTexture,
    bob,
    box,
    clip,
    defaultClip,
    followThrough,
    mat,
    metadata,
    part,
    swing,
    walkCycle,
  } = api;
  const smooth = style === "blend";

  metadata({
    bodyPlans: ["biped", "other"],
    disposition: "hostile",
    groups: ["fantasy", "monster", "humanoid"],
    habitats: ["air", "underground"],
    scale: "medium",
    themes: [smooth ? "alpha-blend" : "alpha-dither", "ghost", "scary", "spectral", "undead"],
  });

  mat("spirit", smooth
    ? { color: "#b7e9ff", alphaMode: "blend", opacity: 0.42 }
    : { color: "#b7e9ff", alphaMode: "mask", opacity: 0.78, alphaCoverage: "dither" });
  mat("spirit_light", smooth
    ? { color: "#e2f8ff", alphaMode: "blend", opacity: 0.5 }
    : { color: "#e2f8ff", alphaMode: "mask", opacity: 0.86, alphaCoverage: "dither" });
  mat("spirit_dark", smooth
    ? { color: "#588aa3", alphaMode: "blend", opacity: 0.34 }
    : { color: "#588aa3", alphaMode: "mask", opacity: 0.68, alphaCoverage: "dither" });
  mat("void", smooth
    ? { color: "#132634", alphaMode: "blend", opacity: 0.72 }
    : { color: "#132634", alphaMode: "mask", opacity: 0.94, alphaCoverage: "dither" });
  mat("glow", smooth
    ? { color: "#bff8ff", alphaMode: "additive", opacity: 0.95 }
    : { color: "#bff8ff", alphaMode: "mask", opacity: 0.98, alphaCoverage: "dither" });

  asciiTexture("wisp", {
    palette: {
      ".": "transparent",
      "f": "#b7e9ff48",
      "m": "#b7e9ff88",
      "b": "#e2f8ffcc",
    },
    pixels: [".bb.", "bmmf", "mmff", "mff.", "ff..", "f..."],
  });

  part("torso", box({
    at: [0, 0.91, 0],
    size: [0.68, 0.62, 0.42],
    material: "spirit",
  }));
  part("chest", box({
    parent: "torso",
    at: [0, 0.03, -0.26],
    size: [0.48, 0.38, 0.14],
    material: "spirit_light",
  }));
  part("neck", box({
    parent: "torso",
    at: [0, 0.41, -0.01],
    size: [0.28, 0.24, 0.28],
    material: "spirit_dark",
    joint: { pivot: [0, -0.1, 0], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.3, -0.04],
    size: [0.7, 0.56, 0.56],
    material: "spirit",
  }));
  part("eye_l", box({
    parent: "head",
    at: [-0.18, 0.06, -0.32],
    size: [0.14, 0.13, 0.1],
    material: "glow",
  }));
  part("eye_r", box({
    parent: "head",
    at: [0.18, 0.06, -0.32],
    size: [0.14, 0.13, 0.1],
    material: "glow",
  }));
  part("mouth", box({
    parent: "head",
    at: [0, -0.16, -0.32],
    size: [0.24, 0.1, 0.1],
    material: "void",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`arm_${side}`, box({
      parent: "torso",
      at: [sign * 0.42, -0.03, -0.01],
      rot: [-8, 0, sign * -16],
      size: [0.22, 0.58, 0.24],
      material: "spirit",
      joint: { pivot: [0, 0.27, 0], axis: [1, 0, 0] },
    }));
    part(`hand_${side}`, box({
      parent: `arm_${side}`,
      at: [0, -0.34, -0.08],
      size: [0.25, 0.18, 0.3],
      material: "spirit_light",
    }));
  }

  part("skirt", box({
    parent: "torso",
    at: [0, -0.45, 0.03],
    size: [0.76, 0.34, 0.4],
    material: "spirit_dark",
  }));
  part("wisp_l", box({
    parent: "skirt",
    at: [-0.22, -0.3, 0.01],
    rot: [0, 0, -7],
    size: [0.3, 0.38, 0.34],
    material: "spirit",
    faces: { north: { texture: "wisp" }, south: { texture: "wisp" } },
  }));
  part("wisp_r", box({
    parent: "skirt",
    at: [0.22, -0.27, 0.01],
    rot: [0, 0, 8],
    size: [0.3, 0.32, 0.34],
    material: "spirit_light",
    faces: { north: { texture: "wisp" }, south: { texture: "wisp" } },
  }));
  part("aura_tail", box({
    parent: "skirt",
    at: [0, -0.1, 0.28],
    rot: [18, 0, 0],
    size: [0.26, 0.5, 0.12],
    material: "glow",
    faces: { north: { texture: "wisp" }, south: { texture: "wisp" } },
  }));

  walkCycle("spectral_drift", {
    label: "Spectral drift",
    role: "idle",
    fps: 24,
    duration: 1.8,
    loop: true,
    samples: 37,
    tracks: [
      bob("torso", { axis: "y", amount: 0.045, center: 0.02, phase: 0.08 }),
      swing("neck", { axis: "z", degrees: 4, phase: 0.16 }),
      swing("arm_l", { axis: "x", degrees: 10, center: -4, phase: 0.02 }),
      swing("arm_r", { axis: "x", degrees: 10, center: -4, phase: 0.52 }),
      followThrough("aura_tail", {
        source: "torso",
        sourceChannel: "pos",
        sourceAxis: "y",
        axis: "x",
        degrees: 13,
        lag: 0.16,
        overshoot: 0.48,
      }),
    ],
  });
  clip("manifest", {
    label: "Manifest",
    role: "action",
    nextClip: "spectral_drift",
    fps: 30,
    loop: false,
    keys: [
      ["torso", 0, { at: [0, 0, 0] }],
      ["torso", 0.4, { at: [0, 0.11, 0] }],
      ["torso", 0.72, { at: [0, 0.03, 0] }],
      ["torso", 1.2, { at: [0, 0, 0] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.4, { rot: [-14, 0, 0] }],
      ["neck", 0.72, { rot: [7, 0, 0] }],
      ["neck", 1.2, { rot: [0, 0, 0] }],
      ["arm_l", 0, { rot: [0, 0, 0] }],
      ["arm_l", 0.5, { rot: [-48, 0, -18] }],
      ["arm_l", 1.2, { rot: [0, 0, 0] }],
      ["arm_r", 0, { rot: [0, 0, 0] }],
      ["arm_r", 0.5, { rot: [-48, 0, 18] }],
      ["arm_r", 1.2, { rot: [0, 0, 0] }],
      ["aura_tail", 0, { rot: [0, 0, 0] }],
      ["aura_tail", 0.5, { rot: [22, 0, 0] }],
      ["aura_tail", 1.2, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("spectral_drift");
}
