import { figure } from "../../src/dsl";

// A box-only desert scorpion with broad pedipalps, eight two-stage legs, and
// a seven-part tail curling forward to a dark stinger.
export default figure("scorpion", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  contactSwing,
  bob,
  swing,
  followThrough,
}) => {
  mat("sand", "#9a6537");
  mat("sand_light", "#c18a4f");
  mat("sand_dark", "#684126");
  mat("armor", "#7d4e2d");
  mat("claw", "#a96d38");
  mat("leg", "#70452a");
  mat("leg_tip", "#3d2d22");
  mat("stinger", "#251d19");
  mat("eye", "#14110f");

  asciiTexture("face", {
    palette: { ".": "#7d4e2d", "e": "#14110f", "h": "#d7ab72", "d": "#684126" },
    pixels: [
      "..eeee..",
      ".ehhhhe.",
      "..e..e..",
      "...dd...",
      "..dddd..",
    ],
  });
  asciiTexture("armor_ridges", {
    palette: { ".": "#9a6537", "l": "#c18a4f", "d": "#684126", "a": "#7d4e2d" },
    pixels: [
      "llllllllll",
      "l........l",
      ".dd....dd.",
      "..a.dd.a..",
      ".dd....dd.",
      "aaaaaaaaaa",
    ],
  });
  asciiTexture("tail_ridges", {
    palette: { ".": "#7d4e2d", "l": "#c18a4f", "d": "#684126" },
    pixels: [
      "llllllll",
      "l......l",
      ".d.dd.d.",
      "dddddddd",
    ],
  });

  part("abdomen", box({
    at: [0, 0.47, 0.14],
    size: [0.62, 0.3, 0.82],
    material: "sand",
    faces: {
      up: { texture: "armor_ridges" },
      east: { texture: "armor_ridges" },
      west: { texture: "armor_ridges" },
    },
  }));
  part("cephalothorax", box({
    parent: "abdomen",
    at: [0, 0.035, -0.55],
    size: [0.66, 0.32, 0.42],
    material: "sand_light",
    faces: { up: { texture: "armor_ridges" } },
  }));
  part("head", box({
    parent: "cephalothorax",
    at: [0, -0.03, -0.34],
    size: [0.44, 0.24, 0.28],
    material: "armor",
    faces: { north: { texture: "face" } },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`claw_arm_${side}`, box({
      parent: "cephalothorax",
      at: [sign * 0.43, -0.015, -0.14],
      rot: [0, sign * 8, sign * -13],
      size: [0.42, 0.1, 0.12],
      material: "sand_dark",
      joint: { pivot: [sign * -0.2, 0, 0], axis: [0, 1, 0] },
    }));
    part(`claw_forearm_${side}`, box({
      parent: `claw_arm_${side}`,
      at: [sign * 0.21, 0, -0.19],
      rot: [0, sign * -18, 0],
      size: [0.12, 0.11, 0.42],
      material: "claw",
      joint: { pivot: [0, 0, 0.19], axis: [0, 1, 0] },
    }));
    part(`claw_palm_${side}`, box({
      parent: `claw_forearm_${side}`,
      at: [0, 0, -0.31],
      size: [0.28, 0.15, 0.22],
      material: "claw",
      faces: { up: { texture: "armor_ridges" } },
    }));
    part(`pincer_outer_${side}`, box({
      parent: `claw_palm_${side}`,
      at: [sign * 0.08, 0, -0.26],
      rot: [0, sign * -8, 0],
      size: [0.085, 0.085, 0.36],
      material: "sand_dark",
      joint: { pivot: [0, 0, 0.15], axis: [0, 1, 0] },
    }));
    part(`pincer_inner_${side}`, box({
      parent: `claw_palm_${side}`,
      at: [sign * -0.08, 0, -0.25],
      rot: [0, sign * 8, 0],
      size: [0.075, 0.075, 0.33],
      material: "leg_tip",
      joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] },
    }));
  }

  for (const [row, z, yaw] of [
    ["front", -0.27, 28],
    ["front_mid", -0.09, 10],
    ["rear_mid", 0.1, -10],
    ["rear", 0.28, -28],
  ] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "abdomen",
        at: [sign * 0.39, -0.17, z],
        rot: [0, sign * yaw, sign * -17],
        size: [0.5, 0.06, 0.07],
        material: "leg",
        joint: { pivot: [sign * -0.24, 0, 0], axis: [0, 1, 0] },
      }));
      part(`leg_${row}_${side}_tip`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.43, -0.075, 0],
        rot: [0, 0, sign * -12],
        size: [0.4, 0.05, 0.06],
        material: "leg_tip",
      }));
    }
  }

  part("tail_1", box({
    parent: "abdomen",
    at: [0, 0.08, 0.59],
    rot: [-18, 0, 0],
    size: [0.34, 0.26, 0.4],
    material: "sand_dark",
    faces: { up: { texture: "tail_ridges" } },
    joint: { pivot: [0, 0, -0.18], axis: [0, 1, 0] },
  }));
  for (const [index, parent, atZ, rotation, width, height, length, material] of [
    [2, "tail_1", 0.35, -25, 0.3, 0.24, 0.34, "armor"],
    [3, "tail_2", 0.32, -32, 0.26, 0.21, 0.3, "sand_light"],
    [4, "tail_3", 0.28, -38, 0.22, 0.18, 0.26, "armor"],
    [5, "tail_4", 0.24, -32, 0.18, 0.15, 0.22, "sand_dark"],
  ] as const) {
    part(`tail_${index}`, box({
      parent,
      at: [0, 0, atZ],
      rot: [rotation, 0, 0],
      size: [width, height, length],
      material,
      faces: { up: { texture: "tail_ridges" } },
      joint: { pivot: [0, 0, -(length / 2 - 0.01)], axis: [0, 1, 0] },
    }));
  }
  part("stinger_bulb", box({
    parent: "tail_5",
    at: [0, 0, 0.2],
    size: [0.2, 0.2, 0.18],
    material: "stinger",
  }));
  part("stinger", box({
    parent: "stinger_bulb",
    at: [0, -0.02, 0.16],
    rot: [-34, 0, 0],
    size: [0.09, 0.09, 0.2],
    material: "stinger",
  }));

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
      bob("abdomen", { axis: "y", amount: 0.014, center: 0.014, phase: 0.5 }),
      contactSwing("leg_front_l", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.66 }),
      contactSwing("leg_front_mid_r", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.66 }),
      contactSwing("leg_rear_mid_l", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.66 }),
      contactSwing("leg_rear_r", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.66 }),
      contactSwing("leg_front_r", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.66 }),
      contactSwing("leg_front_mid_l", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.66 }),
      contactSwing("leg_rear_mid_r", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.66 }),
      contactSwing("leg_rear_l", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.66 }),
      swing("pincer_outer_l", { axis: "y", degrees: 4, phase: 0.5, frequency: 0.5 }),
      swing("pincer_inner_l", { axis: "y", degrees: -4, phase: 0.5, frequency: 0.5 }),
      swing("pincer_outer_r", { axis: "y", degrees: -4, phase: 0.5, frequency: 0.5 }),
      swing("pincer_inner_r", { axis: "y", degrees: 4, phase: 0.5, frequency: 0.5 }),
      swing("tail_1", { axis: "y", degrees: 4, phase: 0.5 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 6, overshoot: 0.25, lag: 0.1 }),
    ],
  });
});
