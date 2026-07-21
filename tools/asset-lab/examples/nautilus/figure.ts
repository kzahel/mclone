import {
  figure,
  type ClipKey,
  type TransformKey,
} from "../../src/dsl";

// A box-only chambered nautilus with a stepped spiral shell, hooded face,
// lateral eyes, eight two-stage tentacles, and separate retract/emerge actions.
export default figure("nautilus", ({
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
  mat("shell", "#d7c89a");
  mat("shell_light", "#eee3bd");
  mat("shell_dark", "#8b7351");
  mat("stripe", "#9d684c");
  mat("body", "#a88b6a");
  mat("body_light", "#c4aa82");
  mat("body_dark", "#665343");
  mat("tentacle", "#d4b996");
  mat("tentacle_dark", "#9d7e64");
  mat("eye", "#1b1c18");

  asciiTexture("shell_spiral", {
    palette: { ".": "#d7c89a", "l": "#eee3bd", "d": "#8b7351", "s": "#9d684c" },
    pixels: [
      "dddddddddddd",
      "dllssssssssd",
      "dls......ssd",
      "dls.dddd.ssd",
      "dls.dlld.ssd",
      "dls.d.ld.ssd",
      "dls.dddd.ssd",
      "dls......ssd",
      "dllssssssssd",
      "dddddddddddd",
    ],
  });
  asciiTexture("hood_bands", {
    palette: { ".": "#a88b6a", "l": "#c4aa82", "d": "#665343", "s": "#9d684c" },
    pixels: ["llllllll", "l......l", "ss....ss", "ssssssss", "d......d", "dddddddd"],
  });
  asciiTexture("tentacle_bands", {
    palette: { ".": "#d4b996", "d": "#9d7e64", "l": "#eee3bd" },
    pixels: ["llll", "....", "dddd", "....", "dddd", "...."],
  });

  part("shell", box({
    at: [0, 1.02, 0.12],
    size: [1.04, 0.94, 0.82],
    material: "shell",
    faces: { east: { texture: "shell_spiral" }, west: { texture: "shell_spiral" } },
  }));
  part("shell_top", box({
    parent: "shell",
    at: [0, 0.49, 0.02],
    size: [0.82, 0.18, 0.66],
    material: "shell_light",
  }));
  part("shell_lower", box({
    parent: "shell",
    at: [0, -0.49, 0.03],
    size: [0.82, 0.18, 0.62],
    material: "shell_dark",
  }));
  part("shell_rear", box({
    parent: "shell",
    at: [0, 0, 0.46],
    size: [0.82, 0.68, 0.22],
    material: "stripe",
  }));
  part("aperture", box({
    parent: "shell",
    at: [0, -0.06, -0.45],
    size: [0.7, 0.58, 0.16],
    material: "body_dark",
  }));
  part("head", box({
    parent: "shell",
    at: [0, -0.08, -0.57],
    size: [0.62, 0.5, 0.44],
    material: "body",
    joint: { pivot: [0, 0, 0.2], axis: [0, 1, 0] },
  }));
  part("hood", box({
    parent: "head",
    at: [0, 0.24, -0.05],
    size: [0.72, 0.24, 0.42],
    material: "body_light",
    faces: { up: { texture: "hood_bands" }, north: { texture: "hood_bands" } },
  }));
  part("mouth", box({
    parent: "head",
    at: [0, -0.13, -0.25],
    size: [0.26, 0.16, 0.14],
    material: "body_dark",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.31, 0.06, -0.08],
      size: [0.15, 0.16, 0.18],
      material: "body_light",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [sign * 0.08, 0.01, -0.03],
      size: [0.08, 0.09, 0.1],
      material: "eye",
    }));
  }

  const tentacleRows = [
    ["upper", 0.13, 0.02],
    ["mid_upper", 0.04, 0.1],
    ["mid_lower", -0.06, 0.16],
    ["lower", -0.15, 0.22],
  ] as const;
  for (let rowIndex = 0; rowIndex < tentacleRows.length; rowIndex += 1) {
    const [row, y, spread] = tentacleRows[rowIndex]!;
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`tentacle_${row}_${side}`, box({
        parent: "head",
        at: [sign * (0.12 + rowIndex * 0.035), y, -0.31],
        rot: [sign * (2 + rowIndex * 2), sign * spread * 40, sign * (rowIndex - 1.5) * 4],
        size: [0.075, 0.075, 0.44],
        material: rowIndex % 2 === 0 ? "tentacle" : "tentacle_dark",
        faces: { east: { texture: "tentacle_bands" }, west: { texture: "tentacle_bands" } },
        joint: { pivot: [0, 0, 0.19], axis: [0, 1, 0] },
      }));
      part(`tentacle_${row}_${side}_tip`, box({
        parent: `tentacle_${row}_${side}`,
        at: [sign * 0.02, sign * (rowIndex - 1.5) * 0.015, -0.31],
        rot: [sign * (rowIndex - 1) * 3, sign * (6 + rowIndex * 3), 0],
        size: [0.055, 0.055, 0.34],
        material: rowIndex % 2 === 0 ? "tentacle_dark" : "tentacle",
        joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] },
      }));
    }
  }

  const hoverTracks = [
    bob("shell", { axis: "y", amount: 0.04, phase: 0.5 }),
    swing("shell", { axis: "y", degrees: 2, phase: 0.25 }),
    swing("head", { axis: "y", degrees: 3.5, phase: 0.72 }),
  ];
  for (let rowIndex = 0; rowIndex < tentacleRows.length; rowIndex += 1) {
    const [row] = tentacleRows[rowIndex]!;
    hoverTracks.push(
      swing(`tentacle_${row}_l`, { axis: "y", degrees: 7 + rowIndex * 2, phase: rowIndex * 0.08 }),
      swing(`tentacle_${row}_r`, { axis: "y", degrees: -(7 + rowIndex * 2), phase: 0.5 + rowIndex * 0.08 }),
      swing(`tentacle_${row}_l_tip`, { axis: "y", degrees: 10 + rowIndex * 2, phase: 0.12 + rowIndex * 0.08 }),
      swing(`tentacle_${row}_r_tip`, { axis: "y", degrees: -(10 + rowIndex * 2), phase: 0.62 + rowIndex * 0.08 }),
    );
  }
  walkCycle("hover", {
    label: "Jet hover",
    role: "locomotion",
    fps: 24,
    duration: 1.28,
    loop: true,
    samples: 33,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.34,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: hoverTracks,
  });

  const retractedPose: Array<readonly [string, TransformKey]> = [
    ["head", { at: [0, 0.03, 0.32], scale: [0.72, 0.72, 0.62] }],
    ["hood", { at: [0, -0.04, 0.08], scale: [0.84, 0.72, 0.78] }],
    ["mouth", { at: [0, 0.05, 0.08], scale: [0.62, 0.62, 0.5] }],
    ["eye_mound_l", { at: [0.08, -0.02, 0.06], scale: [0.55, 0.55, 0.55] }],
    ["eye_mound_r", { at: [-0.08, -0.02, 0.06], scale: [0.55, 0.55, 0.55] }],
  ];
  for (let rowIndex = 0; rowIndex < tentacleRows.length; rowIndex += 1) {
    const [row] = tentacleRows[rowIndex]!;
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      retractedPose.push(
        [`tentacle_${row}_${side}`, {
          at: [sign * -0.06, -rowIndex * 0.018, 0.18 + rowIndex * 0.025],
          rot: [0, sign * (12 + rowIndex * 4), sign * -8],
          scale: [0.55, 0.55, 0.34],
        }],
        [`tentacle_${row}_${side}_tip`, {
          at: [sign * -0.02, 0, 0.12],
          rot: [0, sign * (18 + rowIndex * 5), 0],
          scale: [0.45, 0.45, 0.24],
        }],
      );
    }
  }
  clip("retract", {
    label: "Retract",
    role: "action",
    fps: 24,
    loop: false,
    keys: transitionKeys(retractedPose, 0, 0.2, 0.68),
  });
  clip("emerge", {
    label: "Emerge",
    role: "action",
    nextClip: "hover",
    fps: 24,
    loop: false,
    keys: transitionKeys(retractedPose, 1, 0.46, 0.82),
  });
  defaultClip("hover");
});

function transitionKeys(
  targetPose: ReadonlyArray<readonly [string, TransformKey]>,
  targetAtStart: 0 | 1,
  middleTime: number,
  endTime: number,
): ClipKey[] {
  const keys: ClipKey[] = [];
  const startAmount = targetAtStart;
  const endAmount = 1 - targetAtStart;
  const middleAmount = startAmount + (endAmount - startAmount) * 0.6;
  for (const [partName, transform] of targetPose) {
    keys.push(
      [partName, 0, scaledTransform(transform, startAmount)],
      [partName, middleTime, scaledTransform(transform, middleAmount)],
      [partName, endTime, scaledTransform(transform, endAmount)],
    );
  }
  return keys;
}

function scaledTransform(transform: TransformKey, amount: number): TransformKey {
  return {
    ...(transform.at === undefined
      ? {}
      : { at: transform.at.map((value) => value * amount) as [number, number, number] }),
    ...(transform.rot === undefined
      ? {}
      : { rot: transform.rot.map((value) => value * amount) as [number, number, number] }),
    ...(transform.scale === undefined
      ? {}
      : { scale: transform.scale.map((value) => 1 + (value - 1) * amount) as [number, number, number] }),
  };
}
