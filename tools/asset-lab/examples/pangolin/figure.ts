import { figure } from "../../src/dsl";

// A box-only ground pangolin with five overlapping stepped scale plates, a
// small pointed head, broad digging paws, and a long four-stage armored tail.
// The plates stay rigid while the legs and tail carry a cautious walk.
export default figure("pangolin", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  swing,
  followThrough,
}) => {
  mat("hide", "#76614b");
  mat("hide_light", "#9b7e5c");
  mat("hide_dark", "#4c3f33");
  mat("scale", "#826c50");
  mat("scale_light", "#aa8f67");
  mat("scale_dark", "#5d4e3c");
  mat("belly", "#a98c69");
  mat("nose", "#211c18");
  mat("paw", "#514235");
  mat("claw", "#d7c39a");

  asciiTexture("scale_side", {
    palette: { ".": "#826c50", "l": "#aa8f67", "d": "#5d4e3c", "h": "#4c3f33" },
    pixels: [
      "dddddddddddd",
      "dll..ll..lld",
      "l..dd..dd..l",
      "..dd..dd....",
      ".dd..dd..dd.",
      "hhhhhhhhhhhh",
    ],
  });
  asciiTexture("scale_top", {
    palette: { ".": "#826c50", "l": "#aa8f67", "d": "#5d4e3c" },
    pixels: [
      "dddddddddd",
      "dlllllllll",
      "dl.dd.dd.l",
      "d.dd.dd..l",
      "dd..dd..dl",
      "dddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#76614b", "l": "#9b7e5c", "e": "#171411", "d": "#4c3f33" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      "..llll..",
      "........",
      "........",
    ],
  });
  asciiTexture("snout_tip", {
    palette: { ".": "#76614b", "n": "#211c18" },
    pixels: [
      "........",
      "..nnnn..",
      ".nnnnnn.",
      "..nnnn..",
    ],
  });
  asciiTexture("claw_front", {
    palette: { ".": "#514235", "c": "#d7c39a" },
    pixels: [
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.62, 0.08],
    size: [0.76, 0.46, 1.24],
    material: "hide",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.28, -0.02],
    size: [0.56, 0.11, 0.86],
    material: "belly",
  }));
  for (const [index, z, y, width, height, material] of [
    [1, -0.48, 0.12, 0.68, 0.32, "scale_light"],
    [2, -0.25, 0.18, 0.76, 0.42, "scale"],
    [3, 0, 0.22, 0.82, 0.48, "scale_dark"],
    [4, 0.27, 0.19, 0.8, 0.43, "scale"],
    [5, 0.5, 0.13, 0.7, 0.34, "scale_light"],
  ] as const) {
    part(`scale_plate_${index}`, box({
      parent: "body",
      at: [0, y, z],
      size: [width, height, 0.3],
      material,
      faces: {
        east: { texture: "scale_side" },
        west: { texture: "scale_side" },
        up: { texture: "scale_top" },
      },
    }));
  }

  part("head", box({
    parent: "body",
    at: [0, -0.03, -0.73],
    size: [0.38, 0.34, 0.4],
    material: "hide_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.17], axis: [1, 0, 0] },
  }));
  part("head_plate", box({
    parent: "head",
    at: [0, 0.18, 0.04],
    size: [0.32, 0.11, 0.3],
    material: "scale_light",
    faces: { up: { texture: "scale_top" } },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.08, -0.32],
    size: [0.24, 0.19, 0.28],
    material: "hide",
  }));
  part("snout_tip", box({
    parent: "snout",
    at: [0, -0.02, -0.19],
    size: [0.16, 0.14, 0.14],
    material: "nose",
    faces: { north: { texture: "snout_tip" } },
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.27, -0.32],
    ["fr", 0.27, -0.32],
    ["bl", -0.27, 0.36],
    ["br", 0.27, 0.36],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.35, z],
      size: [0.15, 0.3, 0.17],
      material: "hide_dark",
      joint: { pivot: [0, 0.15, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.19, -0.07],
      size: [0.3, 0.1, 0.34],
      material: "paw",
      faces: { north: { texture: "claw_front" } },
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.02, 0.79],
    size: [0.46, 0.38, 0.54],
    material: "scale_dark",
    faces: {
      east: { texture: "scale_side" },
      west: { texture: "scale_side" },
      up: { texture: "scale_top" },
    },
    joint: { pivot: [0, 0, -0.25], axis: [0, 1, 0] },
  }));
  for (const [index, width, height, length, material] of [
    [2, 0.36, 0.3, 0.5, "scale"],
    [3, 0.25, 0.21, 0.44, "scale_light"],
    [4, 0.14, 0.13, 0.34, "scale_dark"],
  ] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, -0.02, index === 2 ? 0.42 : 0.36],
      size: [width, height, length],
      material,
      faces: { up: { texture: "scale_top" } },
      joint: { pivot: [0, 0, -length / 2], axis: [0, 1, 0] },
    }));
  }

  quadrupedWalk("scuttle", {
    fps: 18,
    duration: 1.14,
    cycleDistance: 0.54,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.01,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 1.8,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.72,
    swingDegrees: 14,
    tracks: [
      swing("tail_1", { axis: "y", degrees: 7, phase: 0.5 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 10, overshoot: 0.4, lag: 0.12 }),
    ],
  });
});
