import { figure } from "../../src/dsl";

// A box-only common seahorse with an upright plated trunk, horse-like snout,
// raised coronet, fluttering dorsal and pectoral fins, and a five-stage curled
// prehensile tail.
export default figure("seahorse", ({
  asciiTexture,
  bob,
  box,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("gold", "#bc8f3f");
  mat("gold_light", "#d7b85f");
  mat("gold_dark", "#74552f");
  mat("belly", "#e0c98a");
  mat("fin", "#c99c55");
  mat("fin_dark", "#8c6538");
  mat("eye", "#d7ca73");
  mat("pupil", "#171510");

  asciiTexture("body_plates", {
    palette: { ".": "#bc8f3f", "l": "#d7b85f", "d": "#74552f", "b": "#e0c98a" },
    pixels: [
      "dddddddd",
      "dll..lld",
      "d......d",
      "dlllllld",
      "d......d",
      "dlllllld",
      "d..bb..d",
      "dddddddd",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#d7b85f", "n": "#74552f", "m": "#4b3b2a" },
    pixels: [
      "........",
      "..nnnn..",
      "........",
      "...mm...",
    ],
  });
  asciiTexture("fin_rays", {
    palette: { ".": "#c99c55", "l": "#e0c98a", "d": "#8c6538" },
    pixels: [
      "dddddddd",
      "dlllll.d",
      "d.l.l..d",
      "dl.l.l.d",
      "d.l.l..d",
      "dddddddd",
    ],
  });
  asciiTexture("tail_rings", {
    palette: { ".": "#bc8f3f", "l": "#d7b85f", "d": "#74552f" },
    pixels: [
      "llllllll",
      "........",
      "dddddddd",
      "........",
      "llllllll",
      "dddddddd",
    ],
  });

  part("trunk", box({
    at: [0, 1.18, 0.08],
    size: [0.5, 0.72, 0.44],
    material: "gold",
    faces: {
      east: { texture: "body_plates" },
      west: { texture: "body_plates" },
    },
  }));
  part("belly", box({
    parent: "trunk",
    at: [0, -0.02, -0.27],
    size: [0.32, 0.58, 0.12],
    material: "belly",
  }));
  part("neck", box({
    parent: "trunk",
    at: [0, 0.48, -0.08],
    rot: [-10, 0, 0],
    size: [0.36, 0.42, 0.32],
    material: "gold_light",
    joint: { pivot: [0, -0.18, 0.1], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.25, -0.18],
    size: [0.46, 0.38, 0.44],
    material: "gold",
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.04, -0.4],
    size: [0.24, 0.2, 0.5],
    material: "gold_light",
    faces: { north: { texture: "snout_face" } },
  }));
  part("snout_tip", box({
    parent: "snout",
    at: [0, 0, -0.29],
    size: [0.2, 0.17, 0.12],
    material: "gold_dark",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.19, 0.08, -0.14],
      size: [0.17, 0.16, 0.18],
      material: "gold_dark",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0.01, -0.105],
      size: [0.1, 0.1, 0.045],
      material: "eye",
      faces: { north: { material: "pupil" } },
    }));
    part(`pectoral_fin_${side}`, box({
      parent: "neck",
      at: [sign * 0.26, -0.06, 0.08],
      rot: [0, sign * -8, sign * 12],
      size: [0.3, 0.05, 0.28],
      material: "fin",
      faces: {
        up: { texture: "fin_rays" },
        down: { texture: "fin_rays" },
      },
      joint: { pivot: [sign * -0.13, 0, 0], axis: [0, 0, 1] },
    }));
  }

  for (const [index, x, height] of [[1, -0.13, 0.25], [2, 0, 0.34], [3, 0.13, 0.25]] as const) {
    part(`coronet_${index}`, box({
      parent: "head",
      at: [x, 0.34, 0.06],
      rot: [0, 0, x < 0 ? -8 : x > 0 ? 8 : 0],
      size: [0.1, height, 0.12],
      material: index === 2 ? "gold_light" : "gold_dark",
    }));
  }

  part("dorsal_fin", box({
    parent: "trunk",
    at: [0, 0.08, 0.35],
    size: [0.07, 0.5, 0.36],
    material: "fin",
    faces: {
      east: { texture: "fin_rays" },
      west: { texture: "fin_rays" },
    },
    joint: { pivot: [0, -0.22, -0.14], axis: [0, 1, 0] },
  }));

  part("tail_1", box({
    parent: "trunk",
    at: [0, -0.5, 0.08],
    size: [0.36, 0.44, 0.36],
    material: "gold_dark",
    faces: { east: { texture: "tail_rings" }, west: { texture: "tail_rings" } },
    joint: { pivot: [0, 0.2, 0], axis: [1, 0, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.34, 0.08],
    rot: [28, 0, 0],
    size: [0.29, 0.36, 0.29],
    material: "gold",
    faces: { east: { texture: "tail_rings" }, west: { texture: "tail_rings" } },
    joint: { pivot: [0, 0.16, 0], axis: [1, 0, 0] },
  }));
  part("tail_3", box({
    parent: "tail_2",
    at: [0, -0.28, 0.08],
    rot: [42, 0, 0],
    size: [0.23, 0.3, 0.23],
    material: "gold_light",
    faces: { east: { texture: "tail_rings" }, west: { texture: "tail_rings" } },
    joint: { pivot: [0, 0.13, 0], axis: [1, 0, 0] },
  }));
  part("tail_4", box({
    parent: "tail_3",
    at: [0, -0.22, 0.06],
    rot: [55, 0, 0],
    size: [0.18, 0.24, 0.18],
    material: "gold_dark",
    joint: { pivot: [0, 0.1, 0], axis: [1, 0, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_4",
    at: [0, -0.16, 0.04],
    rot: [60, 0, 0],
    size: [0.12, 0.18, 0.12],
    material: "gold_light",
    joint: { pivot: [0, 0.07, 0], axis: [1, 0, 0] },
  }));

  walkCycle("hover", {
    label: "Upright hover",
    role: "locomotion",
    fps: 24,
    duration: 1.36,
    loop: true,
    samples: 33,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.32,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("trunk", { axis: "y", amount: 0.045, phase: 0.5 }),
      swing("trunk", { axis: "y", degrees: 2.2, phase: 0.25 }),
      swing("neck", { axis: "y", degrees: 4, phase: 0.75 }),
      swing("dorsal_fin", { axis: "y", degrees: 22, frequency: 3, phase: 0.1 }),
      swing("pectoral_fin_l", { axis: "z", degrees: 13, frequency: 2, phase: 0 }),
      swing("pectoral_fin_r", { axis: "z", degrees: -13, frequency: 2, phase: 0 }),
      swing("tail_1", { axis: "y", degrees: 3, phase: 0.05 }),
      swing("tail_2", { axis: "y", degrees: 5, phase: 0.12 }),
      swing("tail_3", { axis: "y", degrees: 7, phase: 0.2 }),
      swing("tail_4", { axis: "y", degrees: 9, phase: 0.28 }),
      swing("tail_tip", { axis: "y", degrees: 11, phase: 0.36 }),
    ],
  });
  defaultClip("hover");
});
