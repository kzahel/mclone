import { figure } from "../../src/dsl";

// A box-only sheep whose oversized wool cuboid is the silhouette rather than
// a cluster of rounded fluff. A pixel edge pattern gives the fleece some loft.
export default figure("sheep", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
}) => {
  mat("wool", "#e8ece8");
  mat("wool_shadow", "#cfd7d0");
  mat("skin", "#6f5b4f");
  mat("hoof", "#302826");

  asciiTexture("face", {
    palette: { ".": "#6f5b4f", "e": "#16110f", "m": "#3f312b" },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "...mm...",
      "...mm...",
      "........",
      "........",
    ],
  });
  asciiTexture("fleece_side", {
    palette: { ".": "#e8ece8", "s": "#cfd7d0", "h": "#f7f8f5" },
    pixels: [
      "shhsshhsshh",
      "hhhhhhhhhhh",
      "hhhhhhhhhhh",
      "hhhhhhhhhhh",
      "hhhhhhhhhhh",
      "shhsshhsshh",
      ".ss..ss..ss",
    ],
  });

  part("body", box({
    at: [0, 0.9, 0],
    size: [1.2, 0.82, 0.98],
    material: "wool",
    faces: {
      east: { texture: "fleece_side" },
      west: { texture: "fleece_side" },
    },
  }));
  part("wool_top", box({
    parent: "body",
    at: [0, 0.46, 0],
    size: [1.08, 0.16, 0.86],
    material: "wool_shadow",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.08, -0.73],
    size: [0.56, 0.5, 0.52],
    material: "skin",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.23], axis: [1, 0, 0] },
  }));
  part("forelock", box({
    parent: "head",
    at: [0, 0.31, -0.02],
    size: [0.5, 0.18, 0.36],
    material: "wool",
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.34, 0.07, 0],
    rot: [0, 0, -8],
    size: [0.2, 0.13, 0.1],
    material: "skin",
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.34, 0.07, 0],
    rot: [0, 0, 8],
    size: [0.2, 0.13, 0.1],
    material: "skin",
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.38, -0.28],
    ["fr", 0.38, -0.28],
    ["bl", -0.38, 0.3],
    ["br", 0.38, 0.3],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.57, z],
      size: [0.16, 0.46, 0.17],
      material: "skin",
      joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.29, -0.01],
      size: [0.18, 0.1, 0.2],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.16, 0.54],
    rot: [-35, 0, 0],
    size: [0.2, 0.28, 0.2],
    material: "wool_shadow",
    joint: { pivot: [0, -0.12, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.05,
    cycleDistance: 0.68,
    gait: "walk",
    loop: true,
    samples: 11,
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
    swingDegrees: 16,
    tail: "tail",
    tailSwingDegrees: 5,
  });
});
