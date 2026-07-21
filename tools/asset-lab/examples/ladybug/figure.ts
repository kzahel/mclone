import { figure } from "../../src/dsl";

// A box-only seven-spotted ladybug with stepped red wing cases, a patterned
// black pronotum, short antennae, and six legs moving in an alternating tripod.
export default figure("ladybug", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  contactSwing,
  bob,
  swing,
}) => {
  mat("red", "#c83a32");
  mat("red_light", "#e05745");
  mat("red_dark", "#842c2a");
  mat("black", "#242321");
  mat("black_light", "#45433e");
  mat("cream", "#e7ddbd");
  mat("eye", "#6b8e93");

  asciiTexture("face", {
    palette: { ".": "#242321", "e": "#6b8e93", "h": "#b8d1d0" },
    pixels: [
      "........",
      ".ee..ee.",
      ".eh..he.",
      ".ee..ee.",
      "........",
      "........",
    ],
  });
  asciiTexture("pronotum", {
    palette: { ".": "#242321", "l": "#45433e", "c": "#e7ddbd" },
    pixels: [
      "llllllll",
      "l......l",
      "l.cccc.l",
      "l.c..c.l",
      "l......l",
      "........",
    ],
  });
  asciiTexture("shell_spots", {
    palette: {
      ".": "#c83a32",
      "l": "#e05745",
      "d": "#842c2a",
      "b": "#242321",
    },
    pixels: [
      "llllllllll",
      "l........l",
      "..bb......",
      "..bb..bb..",
      "......bb..",
      ".bb....bb.",
      ".bb....bb.",
      "....bb....",
      "....bb....",
      "dddddddddd",
    ],
  });

  part("abdomen", box({
    at: [0, 0.44, 0.12],
    size: [0.64, 0.32, 0.76],
    material: "black",
  }));
  for (const [side, x] of [["l", -0.21], ["r", 0.21]] as const) {
    part(`shell_${side}`, box({
      parent: "abdomen",
      at: [x, 0.22, -0.02],
      size: [0.3, 0.24, 0.62],
      material: side === "l" ? "red" : "red_light",
      faces: {
        up: { texture: "shell_spots" },
        east: { texture: "shell_spots" },
        west: { texture: "shell_spots" },
      },
    }));
  }
  part("thorax", box({
    parent: "abdomen",
    at: [0, 0.02, -0.47],
    size: [0.54, 0.34, 0.34],
    material: "black_light",
    faces: { up: { texture: "pronotum" } },
  }));
  part("head", box({
    parent: "thorax",
    at: [0, -0.01, -0.32],
    size: [0.42, 0.28, 0.3],
    material: "black",
    faces: { north: { texture: "face" } },
  }));
  for (const [side, x, roll] of [["l", -0.13, 25], ["r", 0.13, -25]] as const) {
    part(`antenna_${side}`, box({
      parent: "head",
      at: [x, 0.18, -0.08],
      rot: [-18, 0, roll],
      size: [0.035, 0.26, 0.035],
      material: "black",
    }));
  }

  for (const [row, z, yaw] of [
    ["front", -0.22, 22],
    ["mid", 0, 0],
    ["rear", 0.22, -22],
  ] as const) {
    for (const [side, x, roll] of [["l", -0.39, 16], ["r", 0.39, -16]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "abdomen",
        at: [x, -0.18, z],
        rot: [0, side === "l" ? -yaw : yaw, roll],
        size: [0.46, 0.05, 0.055],
        material: "black",
        joint: {
          pivot: [side === "l" ? 0.22 : -0.22, 0, 0],
          axis: [0, 1, 0],
        },
      }));
    }
  }

  walkCycle("crawl", {
    fps: 20,
    duration: 0.9,
    loop: true,
    samples: 21,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.46,
      direction: [0, 0, -1],
      units: "figure",
      contacts: [
        { part: "leg_front_l", phaseStart: 0, phaseEnd: 0.62, role: "front-left", stanceRatio: 0.62 },
        { part: "leg_mid_r", phaseStart: 0, phaseEnd: 0.62, role: "middle-right", stanceRatio: 0.62 },
        { part: "leg_rear_l", phaseStart: 0, phaseEnd: 0.62, role: "rear-left", stanceRatio: 0.62 },
        { part: "leg_front_r", phaseStart: 0.5, phaseEnd: 0.12, role: "front-right", stanceRatio: 0.62 },
        { part: "leg_mid_l", phaseStart: 0.5, phaseEnd: 0.12, role: "middle-left", stanceRatio: 0.62 },
        { part: "leg_rear_r", phaseStart: 0.5, phaseEnd: 0.12, role: "rear-right", stanceRatio: 0.62 },
      ],
    },
    tracks: [
      bob("abdomen", { axis: "y", amount: 0.014, center: 0.014, phase: 0.5 }),
      contactSwing("leg_front_l", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.62 }),
      contactSwing("leg_mid_r", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.62 }),
      contactSwing("leg_rear_l", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.62 }),
      contactSwing("leg_front_r", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.62 }),
      contactSwing("leg_mid_l", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.62 }),
      contactSwing("leg_rear_r", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.62 }),
      swing("antenna_l", { axis: "z", degrees: 5, phase: 0.15 }),
      swing("antenna_r", { axis: "z", degrees: -5, phase: 0.15 }),
    ],
  });
});
