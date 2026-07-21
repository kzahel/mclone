import { figure } from "../../src/dsl";

// A box-only carpenter ant with a three-section body, narrow petiole, elbowed
// antennae, small mandibles, and six two-stage legs in a worker tripod march.
export default figure("ant", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  contactSwing,
  bob,
  swing,
}) => {
  mat("black", "#252320");
  mat("black_light", "#45413b");
  mat("brown", "#644234");
  mat("brown_light", "#8b5d42");
  mat("leg", "#302a26");
  mat("eye", "#748f91");

  asciiTexture("face", {
    palette: { ".": "#252320", "e": "#748f91", "h": "#b9ccca", "b": "#644234" },
    pixels: [
      "........",
      ".ee..ee.",
      ".eh..he.",
      ".ee..ee.",
      "..bbbb..",
      "........",
    ],
  });
  asciiTexture("thorax_marks", {
    palette: { ".": "#45413b", "b": "#644234", "l": "#8b5d42", "d": "#252320" },
    pixels: [
      "llllllll",
      "l......l",
      ".bb..bb.",
      "..b..b..",
      ".dddddd.",
      "dddddddd",
    ],
  });
  asciiTexture("abdomen_bands", {
    palette: { ".": "#252320", "l": "#45413b", "b": "#644234" },
    pixels: [
      "llllllllll",
      "l........l",
      "bbbbbbbbbb",
      "..........",
      "bbbbbbbbbb",
      "..........",
      "llllllllll",
    ],
  });

  part("abdomen", box({
    at: [0, 0.48, 0.3],
    size: [0.5, 0.36, 0.6],
    material: "black",
    faces: {
      up: { texture: "abdomen_bands" },
      east: { texture: "abdomen_bands" },
      west: { texture: "abdomen_bands" },
    },
  }));
  part("petiole", box({
    parent: "abdomen",
    at: [0, -0.06, -0.4],
    size: [0.2, 0.18, 0.2],
    material: "brown",
  }));
  part("thorax", box({
    parent: "petiole",
    at: [0, 0.02, -0.28],
    size: [0.4, 0.3, 0.4],
    material: "black_light",
    faces: { up: { texture: "thorax_marks" } },
  }));
  part("head", box({
    parent: "thorax",
    at: [0, -0.02, -0.36],
    size: [0.46, 0.28, 0.36],
    material: "black",
    faces: { north: { texture: "face" } },
  }));

  for (const [side, x, yaw] of [["l", -0.13, -12], ["r", 0.13, 12]] as const) {
    part(`mandible_${side}`, box({
      parent: "head",
      at: [x, -0.06, -0.27],
      rot: [4, yaw, 0],
      size: [0.1, 0.1, 0.24],
      material: "brown_light",
    }));
    part(`antenna_${side}`, box({
      parent: "head",
      at: [x, 0.19, -0.08],
      rot: [-28, 0, side === "l" ? 26 : -26],
      size: [0.035, 0.32, 0.035],
      material: "leg",
      joint: { pivot: [0, -0.15, 0], axis: [0, 0, 1] },
    }));
    part(`antenna_${side}_tip`, box({
      parent: `antenna_${side}`,
      at: [0, 0.23, -0.02],
      rot: [-24, 0, side === "l" ? 12 : -12],
      size: [0.03, 0.26, 0.03],
      material: "leg",
    }));
  }

  for (const [row, z, yaw] of [
    ["front", -0.17, 24],
    ["mid", 0, 0],
    ["rear", 0.17, -24],
  ] as const) {
    for (const [side, x, roll] of [["l", -0.33, 17], ["r", 0.33, -17]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "thorax",
        at: [x, -0.18, z],
        rot: [0, side === "l" ? -yaw : yaw, roll],
        size: [0.46, 0.05, 0.055],
        material: "leg",
        joint: {
          pivot: [side === "l" ? 0.22 : -0.22, 0, 0],
          axis: [0, 1, 0],
        },
      }));
      part(`leg_${row}_${side}_tip`, box({
        parent: `leg_${row}_${side}`,
        at: [side === "l" ? -0.38 : 0.38, -0.07, 0],
        rot: [0, 0, side === "l" ? 10 : -10],
        size: [0.34, 0.045, 0.05],
        material: "leg",
      }));
    }
  }

  walkCycle("march", {
    fps: 20,
    duration: 0.84,
    loop: true,
    samples: 21,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.52,
      direction: [0, 0, -1],
      units: "figure",
      contacts: [
        { part: "leg_front_l_tip", phaseStart: 0, phaseEnd: 0.62, role: "front-left", stanceRatio: 0.62 },
        { part: "leg_mid_r_tip", phaseStart: 0, phaseEnd: 0.62, role: "middle-right", stanceRatio: 0.62 },
        { part: "leg_rear_l_tip", phaseStart: 0, phaseEnd: 0.62, role: "rear-left", stanceRatio: 0.62 },
        { part: "leg_front_r_tip", phaseStart: 0.5, phaseEnd: 0.12, role: "front-right", stanceRatio: 0.62 },
        { part: "leg_mid_l_tip", phaseStart: 0.5, phaseEnd: 0.12, role: "middle-left", stanceRatio: 0.62 },
        { part: "leg_rear_r_tip", phaseStart: 0.5, phaseEnd: 0.12, role: "rear-right", stanceRatio: 0.62 },
      ],
    },
    tracks: [
      bob("abdomen", { axis: "y", amount: 0.016, center: 0.016, phase: 0.5 }),
      contactSwing("leg_front_l", { axis: "y", degrees: 16, phase: 0, stanceRatio: 0.62 }),
      contactSwing("leg_mid_r", { axis: "y", degrees: 16, phase: 0, stanceRatio: 0.62 }),
      contactSwing("leg_rear_l", { axis: "y", degrees: 16, phase: 0, stanceRatio: 0.62 }),
      contactSwing("leg_front_r", { axis: "y", degrees: 16, phase: 0.5, stanceRatio: 0.62 }),
      contactSwing("leg_mid_l", { axis: "y", degrees: 16, phase: 0.5, stanceRatio: 0.62 }),
      contactSwing("leg_rear_r", { axis: "y", degrees: 16, phase: 0.5, stanceRatio: 0.62 }),
      swing("antenna_l", { axis: "z", degrees: 6, phase: 0.1 }),
      swing("antenna_r", { axis: "z", degrees: -6, phase: 0.1 }),
    ],
  });
});
