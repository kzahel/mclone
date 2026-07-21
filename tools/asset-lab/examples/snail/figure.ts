import {
  figure,
  type ClipKey,
  type TransformKey,
} from "../../src/dsl";

// A box-only garden snail with a long mottled foot, stepped spiral shell,
// raised eye stalks, and lower feelers. Its head can retract behind the shell
// aperture and hold there, then emerge into the independent glide loop.
export default figure("snail", ({
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
  mat("body", "#8d8062");
  mat("body_light", "#aaa080");
  mat("body_dark", "#5f5947");
  mat("shell", "#a46f37");
  mat("shell_light", "#c9904a");
  mat("shell_dark", "#5e3d27");
  mat("aperture", "#30271f");
  mat("eye", "#171916");
  mat("eye_glint", "#dce0cc");

  asciiTexture("shell_spiral", {
    palette: { ".": "#a46f37", "l": "#c9904a", "d": "#5e3d27", "h": "#e0ad61" },
    pixels: [
      "dddddddddd",
      "dllhhhhlld",
      "dl......ld",
      "dh.dddd.hd",
      "dh.dlld.hd",
      "dh.d.ld.hd",
      "dh.dddd.hd",
      "dl......ld",
      "dllhhhhlld",
      "dddddddddd",
    ],
  });
  asciiTexture("foot_mottle", {
    palette: { ".": "#8d8062", "l": "#aaa080", "d": "#5f5947" },
    pixels: [
      "llllllllllll",
      "l..d..d....l",
      "..d..d..d...",
      ".d....d...d.",
      "dddddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#8d8062", "l": "#aaa080", "d": "#5f5947" },
    pixels: [
      "ll....ll",
      "........",
      "..dddd..",
      ".d....d.",
      "..llll..",
    ],
  });

  part("foot", box({
    at: [0, 0.14, 0.08],
    size: [0.58, 0.18, 1.5],
    material: "body_dark",
    faces: {
      east: { texture: "foot_mottle" },
      west: { texture: "foot_mottle" },
    },
  }));
  part("tail", box({
    parent: "foot",
    at: [0, -0.01, 0.83],
    size: [0.4, 0.12, 0.34],
    material: "body_dark",
  }));
  part("neck", box({
    parent: "foot",
    at: [0, 0.13, -0.5],
    size: [0.48, 0.28, 0.42],
    material: "body_light",
  }));
  part("head", box({
    parent: "foot",
    at: [0, 0.22, -0.78],
    size: [0.54, 0.36, 0.4],
    material: "body",
    faces: { north: { texture: "face" } },
  }));

  part("shell", box({
    at: [0, 0.82, 0.28],
    size: [0.72, 0.92, 0.94],
    material: "shell",
    faces: {
      east: { texture: "shell_spiral" },
      west: { texture: "shell_spiral" },
    },
  }));
  part("shell_crown", box({
    parent: "shell",
    at: [0, 0.45, 0],
    size: [0.58, 0.2, 0.72],
    material: "shell_light",
  }));
  part("shell_lip", box({
    parent: "shell",
    at: [0, 0, -0.55],
    size: [0.64, 0.68, 0.18],
    material: "shell_dark",
  }));
  part("shell_aperture", box({
    parent: "shell",
    at: [0, -0.17, -0.66],
    size: [0.48, 0.42, 0.1],
    material: "aperture",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_stalk_${side}`, box({
      parent: "head",
      at: [sign * 0.17, 0.29, -0.1],
      rot: [-16, 0, sign * -8],
      size: [0.045, 0.42, 0.045],
      material: "body_light",
      joint: { pivot: [0, -0.19, 0], axis: [1, 0, 0] },
    }));
    part(`eye_${side}`, box({
      parent: `eye_stalk_${side}`,
      at: [0, 0.25, -0.02],
      size: [0.12, 0.12, 0.12],
      material: "eye",
      faces: { north: { material: "eye_glint" } },
    }));
    part(`feeler_${side}`, box({
      parent: "head",
      at: [sign * 0.17, -0.01, -0.26],
      rot: [0, sign * 12, 0],
      size: [0.035, 0.035, 0.3],
      material: "body_light",
      joint: { pivot: [0, 0, 0.13], axis: [0, 1, 0] },
    }));
  }

  walkCycle("glide", {
    label: "Slow glide",
    role: "locomotion",
    fps: 20,
    duration: 1.6,
    loop: true,
    samples: 33,
    locomotion: {
      kind: "slither",
      cycleDistance: 0.18,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("head", { axis: "y", amount: 0.014, center: 0.014, phase: 0.25 }),
      swing("head", { axis: "y", degrees: 1.8, frequency: 0.5 }),
      swing("eye_stalk_l", { axis: "z", degrees: 4, phase: 0.05 }),
      swing("eye_stalk_r", { axis: "z", degrees: -4, phase: 0.55 }),
      swing("feeler_l", { axis: "y", degrees: 7, phase: 0.1 }),
      swing("feeler_r", { axis: "y", degrees: -7, phase: 0.6 }),
    ],
  });

  const retractedPose: Array<readonly [string, TransformKey]> = [
    ["shell", { at: [0, -0.31, 0] }],
    // The foot's rest bottom is y=0.05. Pairing this translation with the
    // 0.4 Y scale keeps that bottom exactly fixed for every interpolated pose.
    ["foot", { at: [0, -0.054, 0.18], scale: [0.7, 0.4, 0.25] }],
    ["neck", { at: [0, 0.2, 0.28], scale: [0.8, 0.25, 0.2] }],
    ["head", { at: [0, 0.3, 0.5], scale: [0.68, 0.68, 0.68] }],
    ["eye_stalk_l", { rot: [70, 0, 18], scale: [0.15, 0.15, 0.15] }],
    ["eye_stalk_r", { rot: [70, 0, -18], scale: [0.15, 0.15, 0.15] }],
    ["feeler_l", { rot: [0, -62, 0], scale: [0.12, 0.12, 0.12] }],
    ["feeler_r", { rot: [0, 62, 0], scale: [0.12, 0.12, 0.12] }],
  ];
  clip("retract", {
    label: "Retract",
    role: "action",
    fps: 24,
    loop: false,
    keys: transitionKeys(retractedPose, 0, 0.24, 0.72),
  });
  clip("emerge", {
    label: "Emerge",
    role: "action",
    nextClip: "glide",
    fps: 24,
    loop: false,
    keys: transitionKeys(retractedPose, 1, 0.5, 0.86),
  });
  defaultClip("glide");
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
  const middleAmount = startAmount + (endAmount - startAmount) * 0.58;
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
      : {
        scale: transform.scale.map((value) => 1 + (value - 1) * amount) as [number, number, number],
      }),
  };
}
