import { figure, type CycleTrack } from "../../src/dsl";

// A box-only peacock mantis shrimp with stalked turquoise eyes, a vivid plated
// shell, folded orange clubs, six walking legs, swimmerets, and a punch action.
export default figure("mantis_shrimp", ({
  asciiTexture,
  bob,
  box,
  clip,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("green", "#3f8c69");
  mat("green_light", "#70b77c");
  mat("blue", "#397c91");
  mat("purple", "#714d83");
  mat("orange", "#df7850");
  mat("yellow", "#e4bc55");
  mat("cream", "#e2caa4");
  mat("eye", "#172322");
  mat("eye_glint", "#8fd2bd");

  asciiTexture("shell_bands", {
    palette: { ".": "#3f8c69", "l": "#70b77c", "b": "#397c91", "p": "#714d83", "o": "#df7850" },
    pixels: [
      "llllllllll",
      "lbb....bbl",
      "pppppppppp",
      "p..oo..o.p",
      "bbbbbbbbbb",
      "b........b",
      "oooooooooo",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#3f8c69", "b": "#397c91", "p": "#714d83", "y": "#e4bc55" },
    pixels: ["bb......bb", "by......yb", "..pppppp..", ".p......p.", "....yy...."],
  });
  asciiTexture("tail_fan", {
    palette: { ".": "#397c91", "g": "#70b77c", "p": "#714d83", "o": "#df7850", "y": "#e4bc55" },
    pixels: ["yyyyyyyy", "ygg..ggy", "ppoooopp", "p..oo..p", "ggppppgg", "oooooooo"],
  });

  part("carapace", box({
    at: [0, 0.72, -0.32],
    size: [0.68, 0.42, 0.76],
    material: "green",
    faces: {
      up: { texture: "shell_bands" },
      east: { texture: "shell_bands" },
      west: { texture: "shell_bands" },
    },
  }));
  part("head_plate", box({
    parent: "carapace",
    at: [0, 0.02, -0.48],
    size: [0.66, 0.36, 0.3],
    material: "blue",
    faces: { north: { texture: "face" } },
  }));
  part("mouth_plate", box({
    parent: "head_plate",
    at: [0, -0.19, -0.08],
    size: [0.36, 0.12, 0.24],
    material: "cream",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_stalk_${side}`, box({
      parent: "head_plate",
      at: [sign * 0.22, 0.25, -0.05],
      rot: [-6, 0, sign * -9],
      size: [0.055, 0.22, 0.055],
      material: "purple",
      joint: { pivot: [0, -0.1, 0], axis: [0, 0, 1] },
    }));
    part(`eye_${side}`, box({
      parent: `eye_stalk_${side}`,
      at: [0, 0.14, -0.02],
      size: [0.14, 0.13, 0.13],
      material: "eye",
      faces: { north: { material: "eye_glint" } },
    }));
    part(`raptor_upper_${side}`, box({
      parent: "head_plate",
      at: [sign * 0.24, -0.15, -0.15],
      rot: [28, sign * -6, sign * 8],
      size: [0.14, 0.43, 0.15],
      material: "yellow",
      joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] },
    }));
    part(`raptor_club_${side}`, box({
      parent: `raptor_upper_${side}`,
      at: [0, -0.31, -0.04],
      rot: [-24, 0, sign * -5],
      size: [0.22, 0.24, 0.2],
      material: "orange",
      joint: { pivot: [0, 0.1, 0], axis: [1, 0, 0] },
    }));
  }

  const abdomenWidths = [0.62, 0.58, 0.52, 0.46, 0.38] as const;
  for (let index = 1; index <= abdomenWidths.length; index += 1) {
    const width = abdomenWidths[index - 1]!;
    part(`abdomen_${index}`, box({
      ...(index === 1 ? { parent: "carapace" } : { parent: `abdomen_${index - 1}` }),
      at: index === 1 ? [0, 0.01, 0.48] : [0, 0, 0.31],
      rot: [index > 3 ? -4 : -2, 0, 0],
      size: [width, 0.31 - index * 0.02, 0.35],
      material: index % 2 === 0 ? "blue" : "green_light",
      faces: {
        up: { texture: "shell_bands" },
        east: { texture: "shell_bands" },
        west: { texture: "shell_bands" },
      },
      joint: { pivot: [0, 0, -0.16], axis: [0, 1, 0] },
    }));
  }
  part("tail_center", box({
    parent: "abdomen_5",
    at: [0, 0, 0.29],
    rot: [-5, 0, 0],
    size: [0.3, 0.17, 0.34],
    material: "purple",
    joint: { pivot: [0, 0, -0.15], axis: [0, 1, 0] },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`tail_fan_${side}`, box({
      parent: "tail_center",
      at: [sign * 0.2, 0, 0.16],
      rot: [0, sign * -22, 0],
      size: [0.3, 0.08, 0.4],
      material: "blue",
      faces: { up: { texture: "tail_fan" }, down: { texture: "tail_fan" } },
      joint: { pivot: [sign * -0.12, 0, -0.14], axis: [0, 1, 0] },
    }));
  }

  const legRows = [["front", -0.2], ["middle", 0], ["rear", 0.2]] as const;
  for (const [row, z] of legRows) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "carapace",
        at: [sign * 0.4, -0.2, z],
        rot: [0, sign * -10, -sign * 18],
        size: [0.4, 0.055, 0.065],
        material: row === "middle" ? "purple" : "orange",
        joint: { pivot: [sign * -0.18, 0, 0], axis: [0, 0, 1] },
      }));
      part(`leg_${row}_${side}_tip`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.3, -0.09, -0.03],
        rot: [0, 0, -sign * 18],
        size: [0.26, 0.04, 0.05],
        material: "yellow",
      }));
    }
  }

  for (let index = 1; index <= abdomenWidths.length; index += 1) {
    const width = abdomenWidths[index - 1]!;
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`swimmeret_${index}_${side}`, box({
        parent: `abdomen_${index}`,
        at: [sign * (width / 2 - 0.05), -0.15, 0],
        rot: [8, sign * -8, -sign * 17],
        size: [0.2, 0.04, 0.22],
        material: index % 2 === 0 ? "orange" : "cream",
        joint: { pivot: [sign * -0.08, 0, -0.08], axis: [0, 0, 1] },
      }));
    }
  }

  const tracks: CycleTrack[] = [
    bob("carapace", { axis: "y", amount: 0.018, phase: 0.5 }),
    swing("carapace", { axis: "y", degrees: 1.8, phase: 0 }),
    swing("eye_stalk_l", { axis: "z", degrees: 5, phase: 0.08 }),
    swing("eye_stalk_r", { axis: "z", degrees: -5, phase: 0.58 }),
    swing("raptor_upper_l", { axis: "x", degrees: 3, phase: 0.1 }),
    swing("raptor_upper_r", { axis: "x", degrees: 3, phase: 0.1 }),
  ];
  for (let index = 1; index <= abdomenWidths.length; index += 1) {
    const phase = index * 0.055;
    tracks.push(
      swing(`abdomen_${index}`, { axis: "y", degrees: 2.5 + index, phase }),
      swing(`swimmeret_${index}_l`, { axis: "z", degrees: 16, frequency: 2, phase }),
      swing(`swimmeret_${index}_r`, { axis: "z", degrees: -16, frequency: 2, phase }),
    );
  }
  for (let rowIndex = 0; rowIndex < legRows.length; rowIndex += 1) {
    const [row] = legRows[rowIndex]!;
    tracks.push(
      swing(`leg_${row}_l`, { axis: "z", degrees: 8, frequency: 2, phase: rowIndex * 0.12 }),
      swing(`leg_${row}_r`, { axis: "z", degrees: -8, frequency: 2, phase: rowIndex * 0.12 + 0.5 }),
    );
  }
  tracks.push(
    swing("tail_center", { axis: "y", degrees: 7, phase: 0.35 }),
    swing("tail_fan_l", { axis: "y", degrees: 9, phase: 0.43 }),
    swing("tail_fan_r", { axis: "y", degrees: 9, phase: 0.43 }),
  );

  walkCycle("reef_scuttle", {
    label: "Reef scuttle",
    role: "locomotion",
    fps: 24,
    duration: 0.9,
    loop: true,
    samples: 25,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.62,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks,
  });
  clip("punch", {
    label: "Raptorial punch",
    role: "action",
    nextClip: "reef_scuttle",
    fps: 30,
    loop: false,
    keys: [
      ["carapace", 0, { at: [0, 0, 0] }],
      ["carapace", 0.14, { at: [0, 0, 0.05] }],
      ["carapace", 0.2, { at: [0, 0, -0.05] }],
      ["carapace", 0.36, { at: [0, 0, 0.03] }],
      ["carapace", 0.64, { at: [0, 0, 0] }],
      ["raptor_upper_l", 0, { rot: [0, 0, 0] }],
      ["raptor_upper_l", 0.14, { rot: [-14, 0, 0] }],
      ["raptor_upper_l", 0.2, { rot: [58, 0, 0] }],
      ["raptor_upper_l", 0.3, { rot: [58, 0, 0] }],
      ["raptor_upper_l", 0.46, { rot: [0, 0, 0] }],
      ["raptor_upper_l", 0.64, { rot: [0, 0, 0] }],
      ["raptor_upper_r", 0, { rot: [0, 0, 0] }],
      ["raptor_upper_r", 0.14, { rot: [-14, 0, 0] }],
      ["raptor_upper_r", 0.2, { rot: [58, 0, 0] }],
      ["raptor_upper_r", 0.3, { rot: [58, 0, 0] }],
      ["raptor_upper_r", 0.46, { rot: [0, 0, 0] }],
      ["raptor_upper_r", 0.64, { rot: [0, 0, 0] }],
      ["raptor_club_l", 0, { rot: [0, 0, 0] }],
      ["raptor_club_l", 0.14, { rot: [20, 0, 0] }],
      ["raptor_club_l", 0.2, { rot: [-28, 0, 0] }],
      ["raptor_club_l", 0.3, { rot: [-28, 0, 0] }],
      ["raptor_club_l", 0.46, { rot: [0, 0, 0] }],
      ["raptor_club_l", 0.64, { rot: [0, 0, 0] }],
      ["raptor_club_r", 0, { rot: [0, 0, 0] }],
      ["raptor_club_r", 0.14, { rot: [20, 0, 0] }],
      ["raptor_club_r", 0.2, { rot: [-28, 0, 0] }],
      ["raptor_club_r", 0.3, { rot: [-28, 0, 0] }],
      ["raptor_club_r", 0.46, { rot: [0, 0, 0] }],
      ["raptor_club_r", 0.64, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("reef_scuttle");
});
