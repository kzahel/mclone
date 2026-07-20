import { figure } from "../../src/dsl";

// A box-only king cobra with a raised hood, blunt head, small fangs, forked
// tongue, and a seven-stage ground chain. The shared slither helper sends a
// lateral wave down that parented chain while the hood sways independently.
export default figure("king_cobra", ({
  mat,
  asciiTexture,
  part,
  box,
  slither,
  swing,
}) => {
  mat("scale", "#79643c");
  mat("scale_light", "#a38a55");
  mat("scale_dark", "#3e3b27");
  mat("hood", "#5b4c31");
  mat("hood_light", "#c2a86b");
  mat("belly", "#d0b979");
  mat("eye", "#e2c55c");
  mat("pupil", "#17150e");
  mat("fang", "#eee3c3");
  mat("tongue", "#a83f4f");

  asciiTexture("hood_front", {
    palette: { ".": "#5b4c31", "l": "#c2a86b", "d": "#3e3b27", "b": "#d0b979" },
    pixels: [
      "ddd......ddd",
      "dlll....llld",
      "dl.dd..dd.ld",
      "dl.dd..dd.ld",
      "dlll....llld",
      "dd........dd",
      ".d..bbbb..d.",
      "..bbbbbbbb..",
      "..bbbbbbbb..",
      "...bbbbbb...",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#79643c", "e": "#e2c55c", "p": "#17150e", "d": "#3e3b27" },
    pixels: [
      "dd....dd",
      ".ep..pe.",
      ".ep..pe.",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("scale_bands", {
    palette: { ".": "#79643c", "l": "#a38a55", "d": "#3e3b27", "b": "#d0b979" },
    pixels: [
      "dddddddd",
      "ll....ll",
      "........",
      "..llll..",
      "........",
      "bb....bb",
    ],
  });

  part("body_1", box({
    at: [0, 0.28, 0.16],
    size: [0.42, 0.26, 0.64],
    material: "scale",
    faces: {
      east: { texture: "scale_bands" },
      west: { texture: "scale_bands" },
    },
    joint: { pivot: [0, 0, -0.3], axis: [0, 1, 0] },
  }));
  for (const [name, parent, z, width, height, length, material] of [
    ["body_2", "body_1", 0.54, 0.4, 0.25, 0.62, "scale_light"],
    ["body_3", "body_2", 0.52, 0.36, 0.23, 0.58, "scale"],
    ["body_4", "body_3", 0.49, 0.32, 0.21, 0.54, "scale_dark"],
    ["tail_1", "body_4", 0.45, 0.27, 0.18, 0.48, "scale"],
    ["tail_2", "tail_1", 0.4, 0.21, 0.15, 0.42, "scale_light"],
    ["tail_tip", "tail_2", 0.34, 0.14, 0.11, 0.34, "scale_dark"],
  ] as const) {
    part(name, box({
      parent,
      at: [0, -0.01, z],
      size: [width, height, length],
      material,
      faces: {
        east: { texture: "scale_bands" },
        west: { texture: "scale_bands" },
      },
      joint: { pivot: [0, 0, -length * 0.47], axis: [0, 1, 0] },
    }));
  }

  part("hood", box({
    parent: "body_1",
    at: [0, 0.57, -0.2],
    size: [0.86, 0.9, 0.22],
    material: "hood",
    faces: { north: { texture: "hood_front" } },
    joint: { pivot: [0, -0.43, 0.05], axis: [0, 0, 1] },
  }));
  part("head", box({
    parent: "hood",
    at: [0, 0.56, -0.04],
    size: [0.46, 0.4, 0.46],
    material: "scale",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.09, -0.31],
    size: [0.38, 0.22, 0.2],
    material: "scale_light",
  }));
  for (const [side, x] of [["l", -0.11], ["r", 0.11]] as const) {
    part(`fang_${side}`, box({
      parent: "muzzle",
      at: [x, -0.15, -0.07],
      size: [0.045, 0.16, 0.045],
      material: "fang",
    }));
  }
  part("tongue", box({
    parent: "muzzle",
    at: [0, -0.13, -0.2],
    size: [0.055, 0.045, 0.28],
    material: "tongue",
    joint: { pivot: [0, 0, 0.13], axis: [0, 1, 0] },
  }));
  part("tongue_l", box({
    parent: "tongue",
    at: [-0.045, 0, -0.18],
    rot: [0, -14, 0],
    size: [0.035, 0.035, 0.15],
    material: "tongue",
  }));
  part("tongue_r", box({
    parent: "tongue",
    at: [0.045, 0, -0.18],
    rot: [0, 14, 0],
    size: [0.035, 0.035, 0.15],
    material: "tongue",
  }));

  slither("slither", {
    fps: 18,
    duration: 1.16,
    cycleDistance: 0.74,
    loop: true,
    samples: 21,
    body: "body_1",
    bodyBob: 0.008,
    degrees: 8,
    phaseStep: 0.13,
    segments: ["body_1", "body_2", "body_3", "body_4", "tail_1", "tail_2", "tail_tip"],
    tracks: [
      swing("hood", { axis: "z", degrees: 3, phase: 0.25 }),
      swing("tongue", { axis: "y", degrees: 8, frequency: 2, phase: 0.1 }),
    ],
  });
});
