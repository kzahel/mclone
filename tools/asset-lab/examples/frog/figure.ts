import { figure, type GroundContactConstraint } from "../../src/dsl";

// A box-only green frog with a broad low head, raised amber eyes, squat forelegs,
// and folded three-stage hind legs. The ordinary hop remains separate from a
// larger one-shot jump so gameplay and catalogue callers can select either.
export default figure("frog", ({
  asciiTexture,
  bob,
  box,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("green", "#5f9b45");
  mat("green_light", "#82ba55");
  mat("green_dark", "#355f34");
  mat("belly", "#d5d49a");
  mat("eye", "#cba43b");
  mat("pupil", "#15170f");
  mat("toe", "#d7bd55");

  asciiTexture("face", {
    palette: { ".": "#82ba55", "n": "#355f34", "m": "#263421", "b": "#d5d49a" },
    pixels: [
      "..........",
      "..nn..nn..",
      "..........",
      ".mmmmmmmm.",
      "..bbbbbb..",
    ],
  });
  asciiTexture("back_spots", {
    palette: { ".": "#5f9b45", "l": "#82ba55", "d": "#355f34" },
    pixels: [
      "llllllllll",
      "l..dd....l",
      "l.dd..dd.l",
      "l....dd..l",
      "l.dd....dl",
      "llllllllll",
    ],
  });
  asciiTexture("toe_pads", {
    palette: { ".": "#d7bd55", "g": "#355f34" },
    pixels: [
      ".g.g.g.",
      "ggggggg",
      ".......",
    ],
  });

  part("body", box({
    at: [0, 0.52, 0.08],
    size: [0.72, 0.4, 0.78],
    material: "green",
    faces: {
      up: { texture: "back_spots" },
      east: { texture: "back_spots" },
      west: { texture: "back_spots" },
    },
  }));
  part("back_saddle", box({
    parent: "body",
    at: [0, 0.24, 0.1],
    size: [0.54, 0.12, 0.46],
    material: "green_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.24, -0.08],
    size: [0.56, 0.12, 0.54],
    material: "belly",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.03, -0.47],
    size: [0.78, 0.34, 0.46],
    material: "green_light",
    joint: { pivot: [0, 0, 0.2], axis: [1, 0, 0] },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.08, -0.3],
    size: [0.64, 0.2, 0.2],
    material: "green_light",
    faces: { north: { texture: "face" } },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.26, 0.22, -0.1],
      size: [0.22, 0.18, 0.22],
      material: "green_dark",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0.015, -0.13],
      size: [0.12, 0.12, 0.06],
      material: "eye",
      faces: { north: { material: "pupil" } },
    }));

    part(`arm_${side}`, box({
      parent: "body",
      at: [sign * 0.27, -0.27, -0.27],
      size: [0.14, 0.32, 0.14],
      material: "green_light",
      joint: { pivot: [0, 0.15, 0], axis: [1, 0, 0] },
    }));
    part(`front_foot_${side}`, box({
      parent: `arm_${side}`,
      at: [0, -0.2, -0.1],
      size: [0.22, 0.08, 0.34],
      material: "toe",
      faces: { up: { texture: "toe_pads" } },
    }));

    part(`hind_thigh_${side}`, box({
      parent: "body",
      at: [sign * 0.5, -0.14, 0.23],
      size: [0.42, 0.24, 0.44],
      material: "green_dark",
      joint: { pivot: [sign * -0.18, 0.06, -0.08], axis: [1, 0, 0] },
    }));
    part(`hind_shin_${side}`, box({
      parent: `hind_thigh_${side}`,
      at: [sign * 0.28, -0.18, -0.05],
      size: [0.18, 0.26, 0.46],
      material: "green_light",
      joint: { pivot: [0, 0.12, 0.2], axis: [1, 0, 0] },
    }));
    part(`hind_foot_${side}`, box({
      parent: `hind_shin_${side}`,
      at: [0, -0.16, -0.25],
      size: [0.28, 0.08, 0.46],
      material: "toe",
      faces: { up: { texture: "toe_pads" } },
    }));
  }

  const groundContacts: GroundContactConstraint[] = [
    { contactPart: "front_foot_l", solvePart: "arm_l", axis: "x", minCorrectionDegrees: -60, maxCorrectionDegrees: 60 },
    { contactPart: "front_foot_r", solvePart: "arm_r", axis: "x", minCorrectionDegrees: -60, maxCorrectionDegrees: 60 },
    { contactPart: "hind_foot_l", solvePart: "hind_shin_l", axis: "x", minCorrectionDegrees: -65, maxCorrectionDegrees: 65 },
    { contactPart: "hind_foot_r", solvePart: "hind_shin_r", axis: "x", minCorrectionDegrees: -65, maxCorrectionDegrees: 65 },
  ];

  walkCycle("hop", {
    label: "Hop",
    role: "locomotion",
    fps: 24,
    duration: 0.82,
    groundContacts,
    loop: true,
    samples: 33,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.72,
      direction: [0, 0, -1],
      units: "figure",
      contacts: [
        { part: "front_foot_l", phaseStart: 0.75, phaseEnd: 0.25, role: "front-left", stanceRatio: 0.5 },
        { part: "front_foot_r", phaseStart: 0.75, phaseEnd: 0.25, role: "front-right", stanceRatio: 0.5 },
        { part: "hind_foot_l", phaseStart: 0.75, phaseEnd: 0.25, role: "back-left", stanceRatio: 0.5 },
        { part: "hind_foot_r", phaseStart: 0.75, phaseEnd: 0.25, role: "back-right", stanceRatio: 0.5 },
      ],
    },
    tracks: [
      bob("body", { axis: "y", amount: 0.25, center: 0, phase: 0.5, min: -0.04 }),
      swing("hind_thigh_l", { axis: "x", degrees: 28, phase: 0.5, min: 0 }),
      swing("hind_thigh_r", { axis: "x", degrees: 28, phase: 0.5, min: 0 }),
      swing("hind_shin_l", { axis: "x", degrees: -38, phase: 0.5, max: 0 }),
      swing("hind_shin_r", { axis: "x", degrees: -38, phase: 0.5, max: 0 }),
      swing("arm_l", { axis: "x", degrees: -28, phase: 0.5, max: 0 }),
      swing("arm_r", { axis: "x", degrees: -28, phase: 0.5, max: 0 }),
      swing("head", { axis: "x", degrees: 5, phase: 0.5, min: 0 }),
    ],
  });

  walkCycle("jump", {
    label: "Jump",
    role: "action",
    nextClip: "hop",
    fps: 24,
    duration: 1.06,
    groundContacts,
    loop: false,
    samples: 41,
    tracks: [
      bob("body", { axis: "y", amount: 0.62, center: 0, phase: 0.5, min: -0.04 }),
      swing("hind_thigh_l", { axis: "x", degrees: 36, phase: 0.5, min: 0 }),
      swing("hind_thigh_r", { axis: "x", degrees: 36, phase: 0.5, min: 0 }),
      swing("hind_shin_l", { axis: "x", degrees: -46, phase: 0.5, max: 0 }),
      swing("hind_shin_r", { axis: "x", degrees: -46, phase: 0.5, max: 0 }),
      swing("arm_l", { axis: "x", degrees: -36, phase: 0.5, max: 0 }),
      swing("arm_r", { axis: "x", degrees: -36, phase: 0.5, max: 0 }),
      swing("head", { axis: "x", degrees: 7, phase: 0.5, min: 0 }),
    ],
  });
  defaultClip("hop");
});
