import {
  figure,
  type CycleTrack,
  type LocomotionContactSpec,
} from "../../src/dsl";

// A box-only red rock crab with a broad patterned carapace, raised eyes,
// articulated chelae, and four two-stage walking legs on each side. Its
// locomotion direction is explicitly lateral rather than forward.
export default figure("crab", ({
  asciiTexture,
  bob,
  box,
  contactSwing,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("shell", "#a94732");
  mat("shell_light", "#cf6846");
  mat("shell_dark", "#713127");
  mat("joint", "#7f382b");
  mat("leg", "#9a4431");
  mat("leg_tip", "#542a25");
  mat("eye", "#171a18");
  mat("eye_glint", "#d8e1cf");

  asciiTexture("carapace_top", {
    palette: { ".": "#a94732", "l": "#cf6846", "d": "#713127", "s": "#e18a61" },
    pixels: [
      "dddddddddddd",
      "dlssssssssld",
      "dl..l..l..ld",
      "d..l....l..d",
      "dl...ll...ld",
      "d..l....l..d",
      "dlssssssssld",
      "dddddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#a94732", "d": "#713127", "l": "#cf6846" },
    pixels: [
      "dd........dd",
      "dlll....llld",
      "..d......d..",
      "...dddddd...",
      "....d..d....",
    ],
  });
  asciiTexture("claw_marks", {
    palette: { ".": "#a94732", "l": "#cf6846", "d": "#713127" },
    pixels: [
      "llllllll",
      "l......l",
      "..d..d..",
      ".d....d.",
      "dddddddd",
    ],
  });

  part("carapace", box({
    at: [0, 0.56, 0.08],
    size: [1.12, 0.36, 0.74],
    material: "shell",
    faces: { up: { texture: "carapace_top" } },
  }));
  part("front_plate", box({
    parent: "carapace",
    at: [0, -0.04, -0.43],
    size: [0.94, 0.25, 0.22],
    material: "shell_light",
    faces: { north: { texture: "face" } },
  }));
  part("rear_plate", box({
    parent: "carapace",
    at: [0, -0.07, 0.42],
    size: [0.82, 0.22, 0.18],
    material: "shell_dark",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_stalk_${side}`, box({
      parent: "carapace",
      at: [sign * 0.27, 0.27, -0.3],
      rot: [-8, 0, sign * -5],
      size: [0.075, 0.24, 0.075],
      material: "joint",
      joint: { pivot: [0, -0.1, 0], axis: [0, 0, 1] },
    }));
    part(`eye_${side}`, box({
      parent: `eye_stalk_${side}`,
      at: [0, 0.15, -0.015],
      size: [0.14, 0.13, 0.13],
      material: "eye",
      faces: { north: { material: "eye_glint" } },
    }));

    part(`claw_arm_${side}`, box({
      parent: "carapace",
      at: [sign * 0.58, -0.02, -0.38],
      rot: [0, sign * 22, sign * -5],
      size: [0.5, 0.13, 0.13],
      material: "joint",
      joint: { pivot: [sign * -0.23, 0, 0], axis: [0, 1, 0] },
    }));
    part(`claw_palm_${side}`, box({
      parent: `claw_arm_${side}`,
      at: [sign * 0.4, 0.02, -0.04],
      rot: [0, sign * -8, 0],
      size: [0.36, 0.27, 0.31],
      material: "shell",
      faces: { up: { texture: "claw_marks" } },
    }));
    part(`claw_outer_${side}`, box({
      parent: `claw_palm_${side}`,
      at: [sign * 0.1, 0.035, -0.27],
      rot: [0, sign * 8, 0],
      size: [0.13, 0.13, 0.36],
      material: "shell_light",
      joint: { pivot: [0, 0, 0.16], axis: [0, 1, 0] },
    }));
    part(`claw_inner_${side}`, box({
      parent: `claw_palm_${side}`,
      at: [sign * -0.09, -0.035, -0.25],
      rot: [0, sign * -10, 0],
      size: [0.11, 0.11, 0.32],
      material: "shell_dark",
      joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] },
    }));
  }

  const rows = [
    ["front", -0.27, 20],
    ["front_mid", -0.09, 7],
    ["rear_mid", 0.1, -7],
    ["rear", 0.28, -20],
  ] as const;
  for (const [row, z, yaw] of rows) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "carapace",
        at: [sign * 0.7, -0.17, z],
        rot: [0, sign * yaw, -sign * 12],
        size: [0.54, 0.075, 0.095],
        material: "leg",
        joint: { pivot: [sign * -0.25, 0, 0], axis: [0, 0, 1] },
      }));
      part(`leg_${row}_${side}_tip`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.45, -0.065, 0],
        rot: [0, 0, -sign * 10],
        size: [0.4, 0.055, 0.08],
        material: "leg_tip",
      }));
    }
  }

  const stanceRatio = 0.64;
  const contacts: LocomotionContactSpec[] = [];
  const tracks: CycleTrack[] = [
    bob("carapace", { axis: "y", amount: 0.016, center: 0.016, phase: 0.5 }),
    swing("carapace", { axis: "z", degrees: 1.8, phase: 0.25 }),
    swing("claw_arm_l", { axis: "y", degrees: 5, phase: 0.2 }),
    swing("claw_arm_r", { axis: "y", degrees: -5, phase: 0.7 }),
    swing("claw_outer_l", { axis: "y", degrees: 5, center: -2, frequency: 0.5 }),
    swing("claw_inner_l", { axis: "y", degrees: -5, center: 2, frequency: 0.5 }),
    swing("claw_outer_r", { axis: "y", degrees: -5, center: 2, frequency: 0.5 }),
    swing("claw_inner_r", { axis: "y", degrees: 5, center: -2, frequency: 0.5 }),
  ];
  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const [row] = rows[rowIndex]!;
    const leftPhase = rowIndex * 0.18;
    for (const [side, phase, degrees] of [
      ["l", leftPhase, 13],
      ["r", (leftPhase + 0.5) % 1, -13],
    ] as const) {
      const tipName = `leg_${row}_${side}_tip`;
      contacts.push({
        part: tipName,
        phaseStart: phase,
        phaseEnd: (phase + stanceRatio) % 1,
        role: `${row}-${side}`,
        stanceRatio,
      });
      tracks.push(contactSwing(`leg_${row}_${side}`, {
        axis: "z",
        degrees,
        phase,
        stanceRatio,
      }));
    }
  }

  walkCycle("scuttle", {
    label: "Sideways scuttle",
    role: "locomotion",
    fps: 20,
    duration: 0.92,
    loop: true,
    samples: 21,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.58,
      direction: [1, 0, 0],
      units: "figure",
      contacts,
    },
    tracks,
  });
  defaultClip("scuttle");
});
