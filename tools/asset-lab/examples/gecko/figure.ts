import { figure } from "../../src/dsl";

// A box-only Tokay gecko with blue-gray skin, orange spotting, a broad head,
// four sprawled two-stage legs, adhesive toe pads, and a tapering three-part
// tail. The gait rotates its limbs in the ground plane to preserve the low
// lizard posture.
export default figure("gecko", ({
  asciiTexture,
  bob,
  box,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("skin", "#6f929c");
  mat("skin_light", "#91adb0");
  mat("skin_dark", "#435f68");
  mat("spot", "#db7d3b");
  mat("belly", "#c3c7a9");
  mat("eye", "#d8b548");
  mat("pupil", "#171710");
  mat("toe", "#bfc9b6");

  asciiTexture("spots", {
    palette: { ".": "#6f929c", "o": "#db7d3b", "d": "#435f68", "l": "#91adb0" },
    pixels: [
      "llllllllllll",
      "l.oo...oo..l",
      "l....oo....l",
      "l.oo....oo.l",
      "l...dd.....l",
      "l.oo...oo..l",
      "llllllllllll",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#91adb0", "n": "#435f68", "m": "#2d4148", "o": "#db7d3b" },
    pixels: [
      "o........o",
      "..nn..nn..",
      "..........",
      ".mmmmmmmm.",
    ],
  });
  asciiTexture("toe_pads", {
    palette: { ".": "#bfc9b6", "d": "#435f68" },
    pixels: [
      ".d.d.d.d.",
      "ddddddddd",
      ".........",
    ],
  });

  part("body", box({
    at: [0, 0.38, 0.04],
    size: [0.58, 0.28, 1.1],
    material: "skin",
    faces: {
      up: { texture: "spots" },
      east: { texture: "spots" },
      west: { texture: "spots" },
    },
  }));
  part("back_ridge", box({
    parent: "body",
    at: [0, 0.18, 0.05],
    size: [0.42, 0.1, 0.72],
    material: "skin_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.18, -0.04],
    size: [0.48, 0.1, 0.8],
    material: "belly",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.01, -0.64],
    size: [0.52, 0.24, 0.32],
    material: "skin_light",
    joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.02, -0.34],
    size: [0.64, 0.3, 0.44],
    material: "skin_light",
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.05, -0.29],
    size: [0.54, 0.2, 0.22],
    material: "skin_light",
    faces: { north: { texture: "snout_face" } },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.23, 0.18, -0.06],
      size: [0.2, 0.16, 0.2],
      material: "skin_dark",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0.01, -0.12],
      size: [0.11, 0.1, 0.055],
      material: "eye",
      faces: { north: { material: "pupil" } },
    }));
  }

  for (const [row, z, bend] of [["front", -0.36, -0.1], ["rear", 0.36, 0.1]] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "body",
        at: [sign * 0.42, -0.12, z],
        size: [0.42, 0.12, 0.16],
        material: "skin_dark",
        joint: { pivot: [sign * -0.19, 0, 0], axis: [0, 1, 0] },
      }));
      part(`forearm_${row}_${side}`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.3, -0.1, bend],
        size: [0.38, 0.1, 0.14],
        material: "skin_light",
        joint: { pivot: [sign * -0.17, 0, 0], axis: [0, 1, 0] },
      }));
      part(`foot_${row}_${side}`, box({
        parent: `forearm_${row}_${side}`,
        at: [sign * 0.24, -0.09, bend * 0.8],
        size: [0.28, 0.08, 0.32],
        material: "toe",
        faces: { up: { texture: "toe_pads" } },
      }));
    }
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.02, 0.74],
    size: [0.42, 0.24, 0.62],
    material: "skin",
    faces: {
      up: { texture: "spots" },
      east: { texture: "spots" },
      west: { texture: "spots" },
    },
    joint: { pivot: [0, 0, -0.28], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.02, 0.51],
    size: [0.28, 0.18, 0.54],
    material: "skin_light",
    joint: { pivot: [0, 0, -0.25], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, -0.015, 0.45],
    size: [0.14, 0.12, 0.5],
    material: "skin_dark",
    joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] },
  }));

  walkCycle("scuttle", {
    label: "Sprawled scuttle",
    role: "locomotion",
    fps: 24,
    duration: 0.72,
    loop: true,
    samples: 25,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.62,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("body", { axis: "y", amount: 0.008, center: 0.008, phase: 0.5 }),
      swing("body", { axis: "y", degrees: 1.6, phase: 0 }),
      swing("neck", { axis: "y", degrees: 3, phase: 0.5 }),
      swing("leg_front_l", { axis: "y", degrees: 18, phase: 0 }),
      swing("leg_rear_r", { axis: "y", degrees: -18, phase: 0 }),
      swing("leg_front_r", { axis: "y", degrees: -18, phase: 0.5 }),
      swing("leg_rear_l", { axis: "y", degrees: 18, phase: 0.5 }),
      swing("forearm_front_l", { axis: "y", degrees: -12, phase: 0 }),
      swing("forearm_rear_r", { axis: "y", degrees: 12, phase: 0 }),
      swing("forearm_front_r", { axis: "y", degrees: 12, phase: 0.5 }),
      swing("forearm_rear_l", { axis: "y", degrees: -12, phase: 0.5 }),
      swing("tail_1", { axis: "y", degrees: 8, phase: 0.08 }),
      swing("tail_2", { axis: "y", degrees: 12, phase: 0.18 }),
      swing("tail_tip", { axis: "y", degrees: 16, phase: 0.3 }),
    ],
  });
  defaultClip("scuttle");
});
