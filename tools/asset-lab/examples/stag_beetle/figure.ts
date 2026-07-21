import { figure } from "../../src/dsl";

// A box-only male stag beetle with split chestnut wing cases, a broad armored
// thorax, oversized branched mandibles, clubbed antennae, and six sturdy legs.
export default figure("stag_beetle", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  contactSwing,
  bob,
  swing,
}) => {
  mat("black", "#24211f");
  mat("black_light", "#3d3834");
  mat("chestnut", "#66392c");
  mat("chestnut_light", "#8e5439");
  mat("mandible", "#352824");
  mat("leg", "#2e2926");
  mat("eye", "#6e898a");

  asciiTexture("face", {
    palette: { ".": "#24211f", "e": "#6e898a", "h": "#b5c5bf", "c": "#66392c" },
    pixels: [
      "........",
      ".ee..ee.",
      ".eh..he.",
      ".ee..ee.",
      "..cccc..",
      "........",
    ],
  });
  asciiTexture("thorax_ridges", {
    palette: { ".": "#3d3834", "c": "#66392c", "l": "#8e5439", "b": "#24211f" },
    pixels: [
      "llllllllll",
      "l........l",
      ".cc....cc.",
      "..c....c..",
      ".bbbbbbbb.",
      "bbbbbbbbbb",
    ],
  });
  asciiTexture("shell_ridges", {
    palette: { ".": "#66392c", "l": "#8e5439", "d": "#3d3834", "b": "#24211f" },
    pixels: [
      "llllllllll",
      "l........l",
      ".d.d.d.d..",
      "..d.d.d.d.",
      ".d.d.d.d..",
      "..d.d.d.d.",
      "bbbbbbbbbb",
    ],
  });

  part("abdomen", box({
    at: [0, 0.48, 0.18],
    size: [0.72, 0.36, 0.82],
    material: "black",
  }));
  for (const [side, x] of [["l", -0.23], ["r", 0.23]] as const) {
    part(`shell_${side}`, box({
      parent: "abdomen",
      at: [x, 0.22, 0.02],
      size: [0.32, 0.24, 0.68],
      material: side === "l" ? "chestnut" : "chestnut_light",
      faces: {
        up: { texture: "shell_ridges" },
        east: { texture: "shell_ridges" },
        west: { texture: "shell_ridges" },
      },
    }));
  }
  part("thorax", box({
    parent: "abdomen",
    at: [0, 0.06, -0.52],
    size: [0.64, 0.38, 0.4],
    material: "black_light",
    faces: { up: { texture: "thorax_ridges" } },
  }));
  part("head", box({
    parent: "thorax",
    at: [0, -0.02, -0.37],
    size: [0.58, 0.32, 0.34],
    material: "black",
    faces: { north: { texture: "face" } },
  }));

  for (const [side, x, yaw] of [["l", -0.18, -7], ["r", 0.18, 7]] as const) {
    part(`mandible_${side}`, box({
      parent: "head",
      at: [x, -0.04, -0.34],
      rot: [3, yaw, 0],
      size: [0.14, 0.13, 0.48],
      material: "mandible",
      joint: { pivot: [0, 0, 0.22], axis: [0, 1, 0] },
    }));
    part(`mandible_${side}_tip`, box({
      parent: `mandible_${side}`,
      at: [side === "l" ? -0.04 : 0.04, 0, -0.34],
      rot: [0, side === "l" ? 12 : -12, 0],
      size: [0.11, 0.11, 0.27],
      material: "mandible",
    }));
    part(`mandible_${side}_tooth`, box({
      parent: `mandible_${side}`,
      at: [side === "l" ? 0.11 : -0.11, 0, -0.07],
      rot: [0, side === "l" ? -18 : 18, 0],
      size: [0.2, 0.09, 0.09],
      material: "mandible",
    }));
    part(`antenna_${side}`, box({
      parent: "head",
      at: [side === "l" ? -0.24 : 0.24, 0.18, -0.06],
      rot: [-18, 0, side === "l" ? 28 : -28],
      size: [0.035, 0.26, 0.035],
      material: "leg",
    }));
    part(`antenna_${side}_club`, box({
      parent: `antenna_${side}`,
      at: [0, 0.18, -0.02],
      rot: [-12, 0, side === "l" ? 8 : -8],
      size: [0.07, 0.16, 0.07],
      material: "chestnut",
    }));
  }

  for (const [row, z, yaw] of [
    ["front", -0.2, 22],
    ["mid", 0, 0],
    ["rear", 0.22, -22],
  ] as const) {
    for (const [side, x, roll] of [["l", -0.45, 17], ["r", 0.45, -17]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "abdomen",
        at: [x, -0.2, z],
        rot: [0, side === "l" ? -yaw : yaw, roll],
        size: [0.52, 0.06, 0.065],
        material: "leg",
        joint: {
          pivot: [side === "l" ? 0.25 : -0.25, 0, 0],
          axis: [0, 1, 0],
        },
      }));
      part(`leg_${row}_${side}_tip`, box({
        parent: `leg_${row}_${side}`,
        at: [side === "l" ? -0.43 : 0.43, -0.08, 0],
        rot: [0, 0, side === "l" ? 10 : -10],
        size: [0.38, 0.05, 0.055],
        material: "leg",
      }));
    }
  }

  walkCycle("crawl", {
    fps: 20,
    duration: 1.08,
    loop: true,
    samples: 23,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.58,
      direction: [0, 0, -1],
      units: "figure",
      contacts: [
        { part: "leg_front_l_tip", phaseStart: 0, phaseEnd: 0.64, role: "front-left", stanceRatio: 0.64 },
        { part: "leg_mid_r_tip", phaseStart: 0, phaseEnd: 0.64, role: "middle-right", stanceRatio: 0.64 },
        { part: "leg_rear_l_tip", phaseStart: 0, phaseEnd: 0.64, role: "rear-left", stanceRatio: 0.64 },
        { part: "leg_front_r_tip", phaseStart: 0.5, phaseEnd: 0.14, role: "front-right", stanceRatio: 0.64 },
        { part: "leg_mid_l_tip", phaseStart: 0.5, phaseEnd: 0.14, role: "middle-left", stanceRatio: 0.64 },
        { part: "leg_rear_r_tip", phaseStart: 0.5, phaseEnd: 0.14, role: "rear-right", stanceRatio: 0.64 },
      ],
    },
    tracks: [
      bob("abdomen", { axis: "y", amount: 0.013, center: 0.013, phase: 0.5 }),
      contactSwing("leg_front_l", { axis: "y", degrees: 14, phase: 0, stanceRatio: 0.64 }),
      contactSwing("leg_mid_r", { axis: "y", degrees: 14, phase: 0, stanceRatio: 0.64 }),
      contactSwing("leg_rear_l", { axis: "y", degrees: 14, phase: 0, stanceRatio: 0.64 }),
      contactSwing("leg_front_r", { axis: "y", degrees: 14, phase: 0.5, stanceRatio: 0.64 }),
      contactSwing("leg_mid_l", { axis: "y", degrees: 14, phase: 0.5, stanceRatio: 0.64 }),
      contactSwing("leg_rear_r", { axis: "y", degrees: 14, phase: 0.5, stanceRatio: 0.64 }),
      swing("mandible_l", { axis: "y", degrees: 3, center: -2, frequency: 0.5 }),
      swing("mandible_r", { axis: "y", degrees: -3, center: 2, frequency: 0.5 }),
    ],
  });
});
