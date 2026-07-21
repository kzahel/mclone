import { figure, type GroundContactConstraint } from "../../src/dsl";

// A box-only agile wallaby with a compact gray-brown torso, deep haunches,
// long planted feet, small forearms, and a three-stage balancing tail. The
// synchronized hop uses projected hind-foot contacts at the ground boundary.
export default figure("wallaby", ({
  asciiTexture,
  bob,
  box,
  defaultClip,
  followThrough,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("coat", "#827162");
  mat("coat_light", "#a59178");
  mat("coat_dark", "#524940");
  mat("rust", "#9a6748");
  mat("cream", "#ded0b5");
  mat("muzzle", "#c5aa8d");
  mat("ear_inner", "#bc8880");
  mat("eye", "#171512");
  mat("nose", "#302823");
  mat("claw", "#3b332d");

  asciiTexture("face", {
    palette: { ".": "#827162", "l": "#a59178", "r": "#9a6748", "e": "#171512" },
    pixels: [
      "rr......rr",
      ".ee....ee.",
      "..........",
      "..ll..ll..",
      "..........",
      "..........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#c5aa8d", "n": "#302823", "l": "#ded0b5" },
    pixels: [
      "..nnnn..",
      ".nnnnnn.",
      "...nn...",
      "..llll..",
      ".llllll.",
    ],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#524940", "c": "#3b332d", "l": "#a59178" },
    pixels: [
      "llllllll",
      "........",
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("torso", box({
    at: [0, 1.06, 0.02],
    rot: [-5, 0, 0],
    size: [0.56, 0.84, 0.5],
    material: "coat",
  }));
  part("chest", box({
    parent: "torso",
    at: [0, 0.1, -0.28],
    size: [0.42, 0.56, 0.12],
    material: "cream",
  }));
  part("pelvis", box({
    parent: "torso",
    at: [0, -0.34, 0.16],
    size: [0.72, 0.46, 0.62],
    material: "coat_dark",
  }));
  part("belly", box({
    parent: "torso",
    at: [0, -0.25, -0.29],
    size: [0.4, 0.3, 0.11],
    material: "cream",
  }));

  part("neck", box({
    parent: "torso",
    at: [0, 0.49, -0.02],
    rot: [-8, 0, 0],
    size: [0.34, 0.36, 0.34],
    material: "rust",
    joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.28, -0.1],
    rot: [8, 0, 0],
    size: [0.43, 0.4, 0.43],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.32],
    size: [0.29, 0.21, 0.26],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.13, 0.39, 0.05],
      rot: [-3, 0, sign * -8],
      size: [0.14, 0.5, 0.12],
      material: "coat",
      joint: { pivot: [0, -0.23, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0, -0.071],
      size: [0.07, 0.38, 0.022],
      material: "ear_inner",
    }));

    part(`arm_${side}`, box({
      parent: "torso",
      at: [sign * 0.33, 0.02, -0.08],
      rot: [-20, 0, sign * -6],
      size: [0.16, 0.42, 0.18],
      material: "coat_light",
      joint: { pivot: [0, 0.2, 0], axis: [1, 0, 0] },
    }));
    part(`hand_${side}`, box({
      parent: `arm_${side}`,
      at: [0, -0.27, -0.06],
      size: [0.18, 0.14, 0.27],
      material: "coat_dark",
    }));

    part(`thigh_${side}`, box({
      parent: "torso",
      at: [sign * 0.25, -0.48, 0.12],
      rot: [-16, 0, 0],
      size: [0.34, 0.52, 0.42],
      material: "coat_dark",
      joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${side}`, box({
      parent: `thigh_${side}`,
      at: [0, -0.34, -0.08],
      rot: [22, 0, 0],
      size: [0.22, 0.38, 0.23],
      material: "coat_light",
      joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `shin_${side}`,
      at: [0, -0.24, -0.21],
      size: [0.28, 0.13, 0.62],
      material: "coat_dark",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  part("tail", box({
    parent: "pelvis",
    at: [0, -0.17, 0.32],
    rot: [-54, 0, 0],
    size: [0.28, 0.82, 0.28],
    material: "coat_dark",
    joint: { pivot: [0, 0.39, 0], axis: [1, 0, 0] },
  }));
  part("tail_lower", box({
    parent: "tail",
    at: [0, -0.56, 0],
    rot: [-8, 0, 0],
    size: [0.22, 0.56, 0.22],
    material: "coat_light",
  }));
  part("tail_tip", box({
    parent: "tail_lower",
    at: [0, -0.35, 0],
    size: [0.16, 0.3, 0.16],
    material: "coat_dark",
  }));

  const groundContacts: GroundContactConstraint[] = [
    { contactPart: "foot_l", solvePart: "shin_l", axis: "x", minCorrectionDegrees: -65, maxCorrectionDegrees: 65 },
    { contactPart: "foot_r", solvePart: "shin_r", axis: "x", minCorrectionDegrees: -65, maxCorrectionDegrees: 65 },
  ];
  walkCycle("hop", {
    label: "Bounding hop",
    role: "locomotion",
    fps: 24,
    duration: 0.88,
    groundContacts,
    loop: true,
    samples: 33,
    locomotion: {
      kind: "biped-walk",
      cycleDistance: 0.92,
      direction: [0, 0, -1],
      units: "figure",
      contacts: [
        { part: "foot_l", phaseStart: 0.75, phaseEnd: 0.25, role: "left", stanceRatio: 0.5 },
        { part: "foot_r", phaseStart: 0.75, phaseEnd: 0.25, role: "right", stanceRatio: 0.5 },
      ],
    },
    tracks: [
      bob("torso", { axis: "y", amount: 0.15, center: 0, phase: 0.5, min: -0.035 }),
      swing("thigh_l", { axis: "x", degrees: 24, phase: 0.5, min: 0 }),
      swing("thigh_r", { axis: "x", degrees: 24, phase: 0.5, min: 0 }),
      swing("shin_l", { axis: "x", degrees: -32, phase: 0.5, max: 0 }),
      swing("shin_r", { axis: "x", degrees: -32, phase: 0.5, max: 0 }),
      swing("arm_l", { axis: "x", degrees: -12, center: -10, phase: 0.5 }),
      swing("arm_r", { axis: "x", degrees: -12, center: -10, phase: 0.5 }),
      followThrough("tail", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.45, lag: 0.11 }),
      followThrough("ear_l", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.6, lag: 0.13 }),
      followThrough("ear_r", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.6, lag: 0.13 }),
    ],
  });
  defaultClip("hop");
});
