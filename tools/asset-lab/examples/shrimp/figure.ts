import { figure, type CycleTrack } from "../../src/dsl";

// A box-only pink shrimp with a pointed rostrum, long antennae, small walking
// legs, five paired swimmerets, a curled plated abdomen, and a broad tail fan.
export default figure("shrimp", ({
  asciiTexture,
  bob,
  box,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("shell", "#db806c");
  mat("shell_light", "#f0a58a");
  mat("shell_dark", "#a9504d");
  mat("band", "#bd625a");
  mat("leg", "#e99878");
  mat("leg_tip", "#8f4949");
  mat("eye", "#151716");
  mat("eye_glint", "#e8ecd9");

  asciiTexture("shell_stripes", {
    palette: { ".": "#db806c", "l": "#f0a58a", "d": "#a9504d", "b": "#bd625a" },
    pixels: [
      "llllllllll",
      "l........l",
      "..bb..bb..",
      "b........b",
      "..dddddd..",
      "d........d",
      "dddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#db806c", "l": "#f0a58a", "d": "#a9504d" },
    pixels: [
      "ll......ll",
      "l........l",
      "..d....d..",
      "...dddd...",
      "....dd....",
    ],
  });
  asciiTexture("tail_rays", {
    palette: { ".": "#db806c", "l": "#f0a58a", "d": "#a9504d" },
    pixels: [
      "llllllll",
      "l.d..d.l",
      "l..dd..l",
      "l.d..d.l",
      "l..dd..l",
      "dddddddd",
    ],
  });

  part("carapace", box({
    at: [0, 0.78, -0.38],
    size: [0.52, 0.42, 0.68],
    material: "shell",
    faces: {
      up: { texture: "shell_stripes" },
      east: { texture: "shell_stripes" },
      west: { texture: "shell_stripes" },
    },
  }));
  part("head_plate", box({
    parent: "carapace",
    at: [0, 0.01, -0.4],
    size: [0.48, 0.34, 0.2],
    material: "shell_light",
    faces: { north: { texture: "face" } },
  }));
  part("rostrum", box({
    parent: "head_plate",
    at: [0, 0.05, -0.26],
    rot: [-4, 0, 0],
    size: [0.1, 0.08, 0.4],
    material: "shell_dark",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_stalk_${side}`, box({
      parent: "head_plate",
      at: [sign * 0.18, 0.2, -0.04],
      rot: [-7, 0, sign * -8],
      size: [0.045, 0.18, 0.045],
      material: "shell_dark",
      joint: { pivot: [0, -0.07, 0], axis: [0, 0, 1] },
    }));
    part(`eye_${side}`, box({
      parent: `eye_stalk_${side}`,
      at: [0, 0.11, -0.015],
      size: [0.1, 0.1, 0.1],
      material: "eye",
      faces: { north: { material: "eye_glint" } },
    }));
    part(`antenna_${side}_root`, box({
      parent: "head_plate",
      at: [sign * 0.14, 0.04, -0.18],
      rot: [-4, sign * 11, 0],
      size: [0.035, 0.035, 0.74],
      material: "leg",
      joint: { pivot: [0, 0, 0.34], axis: [0, 1, 0] },
    }));
    part(`antenna_${side}_tip`, box({
      parent: `antenna_${side}_root`,
      at: [sign * 0.07, 0, -0.64],
      rot: [0, sign * 9, 0],
      size: [0.028, 0.028, 0.62],
      material: "leg_tip",
      joint: { pivot: [0, 0, 0.28], axis: [0, 1, 0] },
    }));
  }

  const abdomenWidths = [0.47, 0.43, 0.39, 0.34, 0.28] as const;
  const abdomenRotations = [-4, -5, -7, -9, -11] as const;
  for (let index = 1; index <= abdomenWidths.length; index += 1) {
    part(`abdomen_${index}`, box({
      ...(index === 1 ? { parent: "carapace" } : { parent: `abdomen_${index - 1}` }),
      at: index === 1 ? [0, 0.03, 0.43] : [0, 0, 0.28],
      rot: [abdomenRotations[index - 1]!, 0, 0],
      size: [abdomenWidths[index - 1]!, 0.3 - index * 0.025, 0.32],
      material: index % 2 === 0 ? "shell_light" : "shell",
      faces: {
        up: { texture: "shell_stripes" },
        east: { texture: "shell_stripes" },
        west: { texture: "shell_stripes" },
      },
      joint: { pivot: [0, 0, -0.14], axis: [1, 0, 0] },
    }));
  }

  part("tail_center", box({
    parent: "abdomen_5",
    at: [0, 0, 0.27],
    rot: [-10, 0, 0],
    size: [0.22, 0.15, 0.32],
    material: "shell_dark",
    joint: { pivot: [0, 0, -0.14], axis: [1, 0, 0] },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`tail_fan_${side}`, box({
      parent: "tail_center",
      at: [sign * 0.16, 0, 0.13],
      rot: [0, sign * -20, 0],
      size: [0.25, 0.08, 0.38],
      material: "shell_light",
      faces: { up: { texture: "tail_rays" }, down: { texture: "tail_rays" } },
      joint: { pivot: [sign * -0.1, 0, -0.13], axis: [0, 1, 0] },
    }));
  }

  const legRows = [
    ["front", -0.2],
    ["middle", 0],
    ["rear", 0.2],
  ] as const;
  for (const [row, z] of legRows) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "carapace",
        at: [sign * 0.29, -0.2, z],
        rot: [0, sign * -8, -sign * 18],
        size: [0.3, 0.045, 0.05],
        material: "leg",
        joint: { pivot: [sign * -0.14, 0, 0], axis: [0, 0, 1] },
      }));
      part(`leg_${row}_${side}_tip`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.25, -0.08, -0.03],
        rot: [0, 0, -sign * 18],
        size: [0.23, 0.035, 0.04],
        material: "leg_tip",
      }));
    }
  }

  for (let index = 1; index <= abdomenWidths.length; index += 1) {
    const width = abdomenWidths[index - 1]!;
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`swimmeret_${index}_${side}`, box({
        parent: `abdomen_${index}`,
        at: [sign * (width / 2 - 0.05), -0.16, -0.01],
        rot: [10, sign * -8, -sign * 16],
        size: [0.18, 0.04, 0.2],
        material: "leg",
        faces: { up: { texture: "tail_rays" }, down: { texture: "tail_rays" } },
        joint: { pivot: [sign * -0.07, 0, -0.07], axis: [0, 0, 1] },
      }));
    }
  }

  const tracks: CycleTrack[] = [
    bob("carapace", { axis: "y", amount: 0.022, phase: 0.5 }),
    swing("carapace", { axis: "x", degrees: 1.5, phase: 0.25 }),
    swing("antenna_l_root", { axis: "y", degrees: 8, phase: 0.05 }),
    swing("antenna_r_root", { axis: "y", degrees: -8, phase: 0.55 }),
    swing("antenna_l_tip", { axis: "y", degrees: 13, phase: 0.15 }),
    swing("antenna_r_tip", { axis: "y", degrees: -13, phase: 0.65 }),
  ];
  for (let index = 1; index <= abdomenWidths.length; index += 1) {
    const phase = index * 0.06;
    tracks.push(
      swing(`abdomen_${index}`, {
        axis: "x",
        degrees: 2.5 + index * 0.8,
        phase,
      }),
      swing(`swimmeret_${index}_l`, {
        axis: "z",
        degrees: 18,
        frequency: 2,
        phase: phase + 0.05,
      }),
      swing(`swimmeret_${index}_r`, {
        axis: "z",
        degrees: -18,
        frequency: 2,
        phase: phase + 0.05,
      }),
    );
  }
  tracks.push(
    swing("tail_center", { axis: "x", degrees: 8, phase: 0.36 }),
    swing("tail_fan_l", { axis: "x", degrees: 10, phase: 0.44 }),
    swing("tail_fan_r", { axis: "x", degrees: 10, phase: 0.44 }),
  );
  for (let rowIndex = 0; rowIndex < legRows.length; rowIndex += 1) {
    const [row] = legRows[rowIndex]!;
    tracks.push(
      swing(`leg_${row}_l`, { axis: "z", degrees: 8, frequency: 2, phase: rowIndex * 0.12 }),
      swing(`leg_${row}_r`, { axis: "z", degrees: -8, frequency: 2, phase: rowIndex * 0.12 + 0.5 }),
    );
  }

  walkCycle("paddle_swim", {
    label: "Paddle swim",
    role: "locomotion",
    fps: 24,
    duration: 0.96,
    loop: true,
    samples: 25,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.72,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks,
  });
  defaultClip("paddle_swim");
});
