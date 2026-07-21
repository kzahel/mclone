import { figure } from "../../src/dsl";

// A box-only fire salamander with glossy black skin, yellow warning patches,
// a broad low head, sprawled feet, and a long four-stage swimming tail.
export default figure("salamander", ({
  asciiTexture,
  bob,
  box,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("black", "#202524");
  mat("black_light", "#343b38");
  mat("yellow", "#d5ad2f");
  mat("yellow_light", "#edc94a");
  mat("belly", "#6d694c");
  mat("eye", "#171916");

  asciiTexture("warning_spots", {
    palette: { ".": "#202524", "l": "#343b38", "y": "#d5ad2f", "b": "#edc94a" },
    pixels: [
      "llllllllllll",
      "l.yy....yy.l",
      "l...y......l",
      "l......y...l",
      "l.yy....bb.l",
      "llllllllllll",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#202524", "y": "#d5ad2f", "e": "#171916", "h": "#edc94a" },
    pixels: [
      "yy......yy",
      "y.ee..ee.y",
      "..eh..he..",
      "..........",
      "...yyyy...",
      "..........",
    ],
  });

  part("body", box({
    at: [0, 0.29, 0.04],
    size: [0.56, 0.24, 1.02],
    material: "black",
    faces: {
      up: { texture: "warning_spots" },
      east: { texture: "warning_spots" },
      west: { texture: "warning_spots" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.16, -0.04],
    size: [0.46, 0.1, 0.72],
    material: "belly",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.01, -0.59],
    size: [0.48, 0.18, 0.3],
    material: "black_light",
    joint: { pivot: [0, 0, 0.13], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.02, -0.31],
    size: [0.64, 0.28, 0.42],
    material: "black",
    faces: { north: { texture: "face" } },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.03, -0.27],
    size: [0.56, 0.14, 0.18],
    material: "black_light",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_${side}`, box({
      parent: "head",
      at: [sign * 0.23, 0.14, -0.1],
      size: [0.11, 0.1, 0.08],
      material: "yellow_light",
      faces: { north: { material: "eye" } },
    }));
  }

  for (const [row, z, bend] of [["front", -0.32, -0.06], ["rear", 0.34, 0.06]] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "body",
        at: [sign * 0.39, -0.09, z],
        rot: [0, side === "l" ? -12 : 12, sign * 9],
        size: [0.38, 0.1, 0.13],
        material: "black_light",
        joint: { pivot: [sign * -0.17, 0, 0], axis: [0, 1, 0] },
      }));
      part(`forearm_${row}_${side}`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.29, -0.08, bend],
        rot: [0, 0, sign * 14],
        size: [0.3, 0.08, 0.11],
        material: "yellow",
        joint: { pivot: [sign * -0.14, 0, 0], axis: [0, 1, 0] },
      }));
      part(`foot_${row}_${side}`, box({
        parent: `forearm_${row}_${side}`,
        at: [sign * 0.19, -0.045, bend],
        size: [0.22, 0.055, 0.24],
        material: "black",
      }));
    }
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.01, 0.7],
    size: [0.42, 0.2, 0.58],
    material: "black",
    faces: { up: { texture: "warning_spots" } },
    joint: { pivot: [0, 0, -0.26], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, 0, 0.5],
    size: [0.32, 0.17, 0.5],
    material: "black_light",
    joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] },
  }));
  part("tail_3", box({
    parent: "tail_2",
    at: [0, 0, 0.43],
    size: [0.22, 0.14, 0.44],
    material: "yellow",
    joint: { pivot: [0, 0, -0.2], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_3",
    at: [0, 0, 0.34],
    size: [0.12, 0.1, 0.32],
    material: "black",
    joint: { pivot: [0, 0, -0.14], axis: [0, 1, 0] },
  }));

  walkCycle("crawl", {
    label: "Low crawl",
    role: "locomotion",
    fps: 20,
    duration: 0.9,
    loop: true,
    samples: 21,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.54,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("body", { axis: "y", amount: 0.006, center: 0.006, phase: 0.5 }),
      swing("body", { axis: "y", degrees: 1.7 }),
      swing("neck", { axis: "y", degrees: 3.4, phase: 0.5 }),
      swing("leg_front_l", { axis: "y", degrees: 16, phase: 0 }),
      swing("leg_rear_r", { axis: "y", degrees: -16, phase: 0 }),
      swing("leg_front_r", { axis: "y", degrees: -16, phase: 0.5 }),
      swing("leg_rear_l", { axis: "y", degrees: 16, phase: 0.5 }),
      swing("forearm_front_l", { axis: "y", degrees: -11, phase: 0 }),
      swing("forearm_rear_r", { axis: "y", degrees: 11, phase: 0 }),
      swing("forearm_front_r", { axis: "y", degrees: 11, phase: 0.5 }),
      swing("forearm_rear_l", { axis: "y", degrees: -11, phase: 0.5 }),
      swing("tail_1", { axis: "y", degrees: 8, phase: 0.08 }),
      swing("tail_2", { axis: "y", degrees: 12, phase: 0.18 }),
      swing("tail_3", { axis: "y", degrees: 15, phase: 0.28 }),
      swing("tail_tip", { axis: "y", degrees: 18, phase: 0.38 }),
    ],
  });
  defaultClip("crawl");
});
