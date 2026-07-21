import { figure } from "../../src/dsl";

// A box-only garden spider with a patterned abdomen, eight-eye face, paired
// fangs and spinnerets, and eight two-stage legs in an alternating wave crawl.
export default figure("spider", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  contactSwing,
  bob,
  swing,
}) => {
  mat("abdomen", "#58463a");
  mat("abdomen_light", "#806b53");
  mat("carapace", "#3e332d");
  mat("carapace_light", "#695448");
  mat("leg", "#4b3b34");
  mat("leg_tip", "#282523");
  mat("orange", "#b55e32");
  mat("eye", "#8ba2a0");

  asciiTexture("face", {
    palette: { ".": "#3e332d", "e": "#8ba2a0", "h": "#d1d7cd", "o": "#b55e32" },
    pixels: [
      ".eeeeee.",
      "ehhhhhhe",
      ".ee..ee.",
      "..e..e..",
      "...oo...",
      "........",
    ],
  });
  asciiTexture("abdomen_pattern", {
    palette: {
      ".": "#58463a",
      "l": "#806b53",
      "d": "#3e332d",
      "o": "#b55e32",
    },
    pixels: [
      "llllllllll",
      "l........l",
      "..dd..dd..",
      ".d..oo..d.",
      "...oooo...",
      ".d..oo..d.",
      "..dd..dd..",
      "dddddddddd",
    ],
  });
  asciiTexture("carapace_marks", {
    palette: { ".": "#3e332d", "l": "#695448", "o": "#b55e32" },
    pixels: [
      "llllllll",
      "l......l",
      "..oooo..",
      ".o....o.",
      "..o..o..",
      "........",
    ],
  });

  part("abdomen", box({
    at: [0, 0.48, 0.3],
    size: [0.64, 0.36, 0.72],
    material: "abdomen",
    faces: {
      up: { texture: "abdomen_pattern" },
      east: { texture: "abdomen_pattern" },
      west: { texture: "abdomen_pattern" },
    },
  }));
  part("cephalothorax", box({
    parent: "abdomen",
    at: [0, -0.06, -0.53],
    size: [0.52, 0.3, 0.4],
    material: "carapace_light",
    faces: { up: { texture: "carapace_marks" } },
  }));
  part("head", box({
    parent: "cephalothorax",
    at: [0, -0.01, -0.34],
    size: [0.42, 0.22, 0.28],
    material: "carapace",
    faces: { north: { texture: "face" } },
  }));
  for (const [side, x] of [["l", -0.11], ["r", 0.11]] as const) {
    part(`fang_${side}`, box({
      parent: "head",
      at: [x, -0.13, -0.18],
      rot: [12, 0, side === "l" ? 5 : -5],
      size: [0.08, 0.18, 0.11],
      material: "orange",
    }));
    part(`spinneret_${side}`, box({
      parent: "abdomen",
      at: [x, -0.08, 0.43],
      size: [0.1, 0.1, 0.16],
      material: "leg_tip",
    }));
  }

  for (const [row, z, yaw] of [
    ["front", -0.25, 30],
    ["front_mid", -0.08, 12],
    ["rear_mid", 0.1, -12],
    ["rear", 0.27, -30],
  ] as const) {
    for (const [side, x, roll] of [["l", -0.39, 18], ["r", 0.39, -18]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "cephalothorax",
        at: [x, -0.16, z],
        rot: [0, side === "l" ? -yaw : yaw, roll],
        size: [0.56, 0.065, 0.075],
        material: "leg",
        joint: {
          pivot: [side === "l" ? 0.27 : -0.27, 0, 0],
          axis: [0, 1, 0],
        },
      }));
      part(`leg_${row}_${side}_tip`, box({
        parent: `leg_${row}_${side}`,
        at: [side === "l" ? -0.47 : 0.47, -0.08, 0],
        rot: [0, 0, side === "l" ? 12 : -12],
        size: [0.46, 0.055, 0.065],
        material: "leg_tip",
      }));
    }
  }

  walkCycle("crawl", {
    fps: 20,
    duration: 1.04,
    loop: true,
    samples: 23,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.62,
      direction: [0, 0, -1],
      units: "figure",
      contacts: [
        { part: "leg_front_l_tip", phaseStart: 0, phaseEnd: 0.66, role: "front-left", stanceRatio: 0.66 },
        { part: "leg_front_mid_r_tip", phaseStart: 0, phaseEnd: 0.66, role: "front-middle-right", stanceRatio: 0.66 },
        { part: "leg_rear_mid_l_tip", phaseStart: 0, phaseEnd: 0.66, role: "rear-middle-left", stanceRatio: 0.66 },
        { part: "leg_rear_r_tip", phaseStart: 0, phaseEnd: 0.66, role: "rear-right", stanceRatio: 0.66 },
        { part: "leg_front_r_tip", phaseStart: 0.5, phaseEnd: 0.16, role: "front-right", stanceRatio: 0.66 },
        { part: "leg_front_mid_l_tip", phaseStart: 0.5, phaseEnd: 0.16, role: "front-middle-left", stanceRatio: 0.66 },
        { part: "leg_rear_mid_r_tip", phaseStart: 0.5, phaseEnd: 0.16, role: "rear-middle-right", stanceRatio: 0.66 },
        { part: "leg_rear_l_tip", phaseStart: 0.5, phaseEnd: 0.16, role: "rear-left", stanceRatio: 0.66 },
      ],
    },
    tracks: [
      bob("abdomen", { axis: "y", amount: 0.018, center: 0.018, phase: 0.5 }),
      swing("abdomen", { axis: "z", degrees: 1.5, phase: 0.25 }),
      contactSwing("leg_front_l", { axis: "y", degrees: 16, phase: 0, stanceRatio: 0.66 }),
      contactSwing("leg_front_mid_r", { axis: "y", degrees: 16, phase: 0, stanceRatio: 0.66 }),
      contactSwing("leg_rear_mid_l", { axis: "y", degrees: 16, phase: 0, stanceRatio: 0.66 }),
      contactSwing("leg_rear_r", { axis: "y", degrees: 16, phase: 0, stanceRatio: 0.66 }),
      contactSwing("leg_front_r", { axis: "y", degrees: 16, phase: 0.5, stanceRatio: 0.66 }),
      contactSwing("leg_front_mid_l", { axis: "y", degrees: 16, phase: 0.5, stanceRatio: 0.66 }),
      contactSwing("leg_rear_mid_r", { axis: "y", degrees: 16, phase: 0.5, stanceRatio: 0.66 }),
      contactSwing("leg_rear_l", { axis: "y", degrees: 16, phase: 0.5, stanceRatio: 0.66 }),
    ],
  });
});
