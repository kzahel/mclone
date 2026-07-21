import {
  figure,
  type CycleTrack,
  type LocomotionContactSpec,
} from "../../src/dsl";

// A box-only American lobster with a heavy carapace, long paired antennae,
// oversized articulated claws, four walking-leg pairs, a plated abdomen, and
// a broad tail fan. Its measured crawl keeps the tail distinct from the legs.
export default figure("lobster", ({
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
  mat("shell", "#8e3c31");
  mat("shell_light", "#bd5c3e");
  mat("shell_dark", "#562b2a");
  mat("joint", "#6f322b");
  mat("leg", "#a34a32");
  mat("leg_tip", "#492726");
  mat("antenna", "#d17a49");
  mat("eye", "#151716");
  mat("eye_glint", "#d7dfcb");

  asciiTexture("carapace_mottle", {
    palette: { ".": "#8e3c31", "l": "#bd5c3e", "d": "#562b2a", "s": "#d17a49" },
    pixels: [
      "dddddddddddd",
      "dlssssssssld",
      "dl..d..d..ld",
      "d..l....l..d",
      "dl...dd...ld",
      "d..l....l..d",
      "dlssssssssld",
      "dddddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#8e3c31", "l": "#bd5c3e", "d": "#562b2a" },
    pixels: [
      "dd........dd",
      "dll......lld",
      "..d..dd..d..",
      "...dddddd...",
      "....d..d....",
    ],
  });
  asciiTexture("claw_spots", {
    palette: { ".": "#8e3c31", "l": "#bd5c3e", "d": "#562b2a" },
    pixels: [
      "dddddddd",
      "dll....d",
      "d..l...d",
      "d....l.d",
      "d.l....d",
      "dddddddd",
    ],
  });
  asciiTexture("abdomen_bands", {
    palette: { ".": "#8e3c31", "l": "#bd5c3e", "d": "#562b2a" },
    pixels: [
      "llllllllll",
      "l........l",
      "..dddddd..",
      "d........d",
      "dddddddddd",
    ],
  });

  part("carapace", box({
    at: [0, 0.58, -0.2],
    size: [0.78, 0.4, 0.9],
    material: "shell",
    faces: {
      up: { texture: "carapace_mottle" },
      east: { texture: "carapace_mottle" },
      west: { texture: "carapace_mottle" },
    },
  }));
  part("head_plate", box({
    parent: "carapace",
    at: [0, -0.01, -0.53],
    size: [0.7, 0.3, 0.22],
    material: "shell_light",
    faces: { north: { texture: "face" } },
  }));
  part("rostrum", box({
    parent: "head_plate",
    at: [0, 0.02, -0.23],
    size: [0.15, 0.12, 0.34],
    material: "shell_dark",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_stalk_${side}`, box({
      parent: "head_plate",
      at: [sign * 0.22, 0.2, -0.08],
      rot: [-8, 0, sign * -6],
      size: [0.065, 0.2, 0.065],
      material: "joint",
      joint: { pivot: [0, -0.08, 0], axis: [0, 0, 1] },
    }));
    part(`eye_${side}`, box({
      parent: `eye_stalk_${side}`,
      at: [0, 0.12, -0.02],
      size: [0.12, 0.11, 0.12],
      material: "eye",
      faces: { north: { material: "eye_glint" } },
    }));

    part(`antenna_${side}_root`, box({
      parent: "head_plate",
      at: [sign * 0.18, 0.04, -0.22],
      rot: [-3, sign * 12, 0],
      size: [0.045, 0.045, 0.7],
      material: "antenna",
      joint: { pivot: [0, 0, 0.32], axis: [0, 1, 0] },
    }));
    part(`antenna_${side}_tip`, box({
      parent: `antenna_${side}_root`,
      at: [sign * 0.08, 0, -0.61],
      rot: [0, sign * 8, 0],
      size: [0.035, 0.035, 0.6],
      material: "antenna",
      joint: { pivot: [0, 0, 0.27], axis: [0, 1, 0] },
    }));

    part(`claw_arm_${side}`, box({
      parent: "carapace",
      at: [sign * 0.43, -0.08, -0.37],
      rot: [0, sign * 28, sign * -7],
      size: [0.52, 0.12, 0.14],
      material: "joint",
      joint: { pivot: [sign * -0.24, 0, 0], axis: [0, 1, 0] },
    }));
    part(`claw_palm_${side}`, box({
      parent: `claw_arm_${side}`,
      at: [sign * 0.4, 0.02, -0.08],
      rot: [0, sign * -10, 0],
      size: [0.42, 0.28, 0.38],
      material: "shell",
      faces: { up: { texture: "claw_spots" } },
    }));
    part(`claw_outer_${side}`, box({
      parent: `claw_palm_${side}`,
      at: [sign * 0.11, 0.04, -0.32],
      rot: [0, sign * 8, 0],
      size: [0.15, 0.14, 0.42],
      material: "shell_light",
      joint: { pivot: [0, 0, 0.19], axis: [0, 1, 0] },
    }));
    part(`claw_inner_${side}`, box({
      parent: `claw_palm_${side}`,
      at: [sign * -0.1, -0.03, -0.29],
      rot: [0, sign * -11, 0],
      size: [0.12, 0.11, 0.36],
      material: "shell_dark",
      joint: { pivot: [0, 0, 0.16], axis: [0, 1, 0] },
    }));
  }

  const abdomenWidths = [0.68, 0.61, 0.54, 0.46, 0.37] as const;
  for (let index = 1; index <= abdomenWidths.length; index += 1) {
    part(`abdomen_${index}`, box({
      ...(index === 1 ? { parent: "carapace" } : { parent: `abdomen_${index - 1}` }),
      at: index === 1 ? [0, -0.05, 0.57] : [0, -0.025, 0.31],
      size: [abdomenWidths[index - 1]!, 0.28 - index * 0.018, 0.36],
      material: index % 2 === 0 ? "shell_light" : "shell",
      faces: { up: { texture: "abdomen_bands" } },
      joint: { pivot: [0, 0, -0.16], axis: [0, 1, 0] },
    }));
  }
  part("tail_center", box({
    parent: "abdomen_5",
    at: [0, -0.03, 0.34],
    size: [0.28, 0.16, 0.38],
    material: "shell_dark",
    faces: { up: { texture: "abdomen_bands" } },
    joint: { pivot: [0, 0, -0.17], axis: [0, 1, 0] },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`tail_fan_${side}`, box({
      parent: "tail_center",
      at: [sign * 0.21, 0, 0.15],
      rot: [0, sign * -18, 0],
      size: [0.32, 0.12, 0.46],
      material: "shell_light",
      faces: { up: { texture: "abdomen_bands" } },
      joint: { pivot: [sign * -0.13, 0, -0.16], axis: [0, 1, 0] },
    }));
  }

  const rows = [
    ["front", -0.26, 18],
    ["front_mid", -0.06, 7],
    ["rear_mid", 0.14, -7],
    ["rear", 0.33, -18],
  ] as const;
  for (const [row, z, yaw] of rows) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "carapace",
        at: [sign * 0.48, -0.18, z],
        rot: [0, sign * yaw, -sign * 15],
        size: [0.48, 0.07, 0.085],
        material: "leg",
        joint: { pivot: [sign * -0.22, 0, 0], axis: [0, 1, 0] },
      }));
      part(`leg_${row}_${side}_tip`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.4, -0.07, 0],
        rot: [0, 0, -sign * 12],
        size: [0.34, 0.05, 0.065],
        material: "leg_tip",
      }));
    }
  }

  const stanceRatio = 0.66;
  const contacts: LocomotionContactSpec[] = [];
  const tracks: CycleTrack[] = [
    bob("carapace", { axis: "y", amount: 0.014, center: 0.014, phase: 0.5 }),
    swing("carapace", { axis: "z", degrees: 1.5, phase: 0.25 }),
    swing("antenna_l_root", { axis: "y", degrees: 7, phase: 0.08 }),
    swing("antenna_r_root", { axis: "y", degrees: -7, phase: 0.58 }),
    swing("antenna_l_tip", { axis: "y", degrees: 11, phase: 0.18 }),
    swing("antenna_r_tip", { axis: "y", degrees: -11, phase: 0.68 }),
    swing("claw_arm_l", { axis: "y", degrees: 4, phase: 0.15 }),
    swing("claw_arm_r", { axis: "y", degrees: -4, phase: 0.65 }),
    swing("claw_outer_l", { axis: "y", degrees: 5, center: -2, frequency: 0.5 }),
    swing("claw_inner_l", { axis: "y", degrees: -5, center: 2, frequency: 0.5 }),
    swing("claw_outer_r", { axis: "y", degrees: -5, center: 2, frequency: 0.5 }),
    swing("claw_inner_r", { axis: "y", degrees: 5, center: -2, frequency: 0.5 }),
  ];
  for (let index = 1; index <= abdomenWidths.length; index += 1) {
    tracks.push(swing(`abdomen_${index}`, {
      axis: "y",
      degrees: 1.5 + index * 0.45,
      phase: 0.42 + index * 0.08,
    }));
  }
  tracks.push(
    swing("tail_center", { axis: "y", degrees: 4.5, phase: 0.88 }),
    swing("tail_fan_l", { axis: "y", degrees: 5, phase: 0.96 }),
    swing("tail_fan_r", { axis: "y", degrees: -5, phase: 0.96 }),
  );
  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const [row] = rows[rowIndex]!;
    const leftPhase = rowIndex * 0.15;
    for (const [side, phase, degrees] of [
      ["l", leftPhase, 15],
      ["r", (leftPhase + 0.5) % 1, -15],
    ] as const) {
      contacts.push({
        part: `leg_${row}_${side}_tip`,
        phaseStart: phase,
        phaseEnd: (phase + stanceRatio) % 1,
        role: `${row}-${side}`,
        stanceRatio,
      });
      tracks.push(contactSwing(`leg_${row}_${side}`, {
        axis: "y",
        degrees,
        phase,
        stanceRatio,
      }));
    }
  }

  walkCycle("crawl", {
    label: "Seabed crawl",
    role: "locomotion",
    fps: 20,
    duration: 1.18,
    loop: true,
    samples: 25,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.48,
      direction: [0, 0, -1],
      units: "figure",
      contacts,
    },
    tracks,
  });
  defaultClip("crawl");
});
