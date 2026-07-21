import {
  figure,
  type ClipKey,
  type CycleTrack,
  type LocomotionContactSpec,
  type TransformKey,
} from "../../src/dsl";

// A box-only common pill bug built around the middle shell band. The front
// and rear halves are separate chains so both ends can curl below the dorsal
// shell instead of rolling the tail over the animal's back.
export default figure("roly_poly", ({
  asciiTexture,
  bob,
  box,
  clip,
  contactSwing,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("shell", "#666a68");
  mat("shell_light", "#8c918c");
  mat("shell_dark", "#424846");
  mat("shell_edge", "#303634");
  mat("belly", "#a29b89");
  mat("leg", "#6c6254");
  mat("eye", "#171a19");

  asciiTexture("shell_band", {
    palette: { ".": "#666a68", "l": "#8c918c", "d": "#424846", "e": "#303634" },
    pixels: [
      "llllllllll",
      "l........l",
      ".d.dd.dd.d",
      "d.dd.dd.d.",
      ".d.dd.dd.d",
      "eeeeeeeeee",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#424846", "e": "#171a19", "h": "#aeb4ac", "l": "#8c918c" },
    pixels: [
      "........",
      ".ee..ee.",
      ".eh..he.",
      "..llll..",
      ".llllll.",
    ],
  });

  const segments = {
    1: [0.62, 0.26, "shell_dark"],
    2: [0.72, 0.3, "shell"],
    3: [0.82, 0.34, "shell_light"],
    4: [0.88, 0.36, "shell"],
    5: [0.84, 0.35, "shell_dark"],
    6: [0.76, 0.31, "shell"],
    7: [0.66, 0.27, "shell_light"],
  } as const;
  const shellFaces = {
    up: { texture: "shell_band" },
    east: { texture: "shell_band" },
    west: { texture: "shell_band" },
  } as const;

  part("segment_4", box({
    at: [0, 0.48, 0],
    size: [segments[4][0], segments[4][1], 0.22],
    material: segments[4][2],
    faces: shellFaces,
  }));

  for (const [index, parent, y, z, pivotZ] of [
    [3, 4, 0, -0.21, 0.1],
    [2, 3, -0.025, -0.21, 0.1],
    [1, 2, -0.025, -0.21, 0.1],
    [5, 4, -0.015, 0.21, -0.1],
    [6, 5, -0.025, 0.21, -0.1],
    [7, 6, -0.02, 0.21, -0.1],
  ] as const) {
    const [width, height, material] = segments[index];
    part(`segment_${index}`, box({
      parent: `segment_${parent}`,
      at: [0, y, z],
      size: [width, height, 0.22],
      material,
      faces: shellFaces,
      joint: { pivot: [0, 0, pivotZ], axis: [1, 0, 0] },
    }));
  }

  for (let index = 1; index <= 7; index += 1) {
    const [width, height] = segments[index as keyof typeof segments];
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${index}_${side}`, box({
        parent: `segment_${index}`,
        at: [sign * (width / 2 + 0.08), -height * 0.34, 0],
        rot: [0, sign * (18 - index * 6), sign * -30],
        size: [0.24, 0.045, 0.05],
        material: index % 2 === 0 ? "leg" : "belly",
        joint: { pivot: [sign * -0.11, 0, 0], axis: [0, 0, 1] },
      }));
    }
  }

  part("head", box({
    parent: "segment_1",
    at: [0, -0.035, -0.2],
    size: [0.5, 0.22, 0.18],
    material: "shell_dark",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.08], axis: [1, 0, 0] },
  }));
  for (const [side, x, roll] of [["l", -0.15, 24], ["r", 0.15, -24]] as const) {
    part(`antenna_${side}`, box({
      parent: "head",
      at: [x, 0.07, -0.14],
      rot: [-30, 0, roll],
      size: [0.03, 0.25, 0.03],
      material: "leg",
    }));
    part(`antenna_${side}_tip`, box({
      parent: `antenna_${side}`,
      at: [0, 0.18, -0.025],
      rot: [-18, 0, side === "l" ? 7 : -7],
      size: [0.025, 0.18, 0.025],
      material: "shell_edge",
    }));
  }
  part("tail_plate", box({
    parent: "segment_7",
    at: [0, -0.025, 0.18],
    size: [0.5, 0.22, 0.14],
    material: "shell_edge",
    faces: { up: { texture: "shell_band" } },
    joint: { pivot: [0, 0, -0.06], axis: [1, 0, 0] },
  }));

  const stanceRatio = 0.68;
  const contacts: LocomotionContactSpec[] = [];
  const tracks: CycleTrack[] = [
    bob("segment_4", { axis: "y", amount: 0.012, center: 0.012, phase: 0.5 }),
    swing("head", { axis: "y", degrees: 2.5, frequency: 0.5 }),
  ];
  for (let index = 1; index <= 7; index += 1) {
    const leftPhase = ((index - 1) * 0.14) % 1;
    for (const [side, phase] of [["l", leftPhase], ["r", (leftPhase + 0.5) % 1]] as const) {
      const partName = `leg_${index}_${side}`;
      contacts.push({
        part: partName,
        phaseStart: phase,
        phaseEnd: (phase + stanceRatio) % 1,
        role: `segment-${index}-${side}`,
        stanceRatio,
      });
      tracks.push(contactSwing(partName, {
        axis: "y",
        degrees: 12,
        phase,
        stanceRatio,
      }));
    }
  }

  walkCycle("crawl", {
    label: "Crawl",
    role: "locomotion",
    fps: 20,
    duration: 1.12,
    loop: true,
    samples: 24,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.42,
      direction: [0, 0, -1],
      units: "figure",
      contacts,
    },
    tracks,
  });

  const curledPose: Array<readonly [string, TransformKey]> = [
    ["segment_4", { at: [0, 0.08, 0] }],
    ["segment_3", { rot: [-40, 0, 0] }],
    ["segment_2", { rot: [-48, 0, 0] }],
    ["segment_1", { rot: [-55, 0, 0] }],
    ["head", { rot: [-48, 0, 0] }],
    ["segment_5", { rot: [40, 0, 0] }],
    ["segment_6", { rot: [48, 0, 0] }],
    ["segment_7", { rot: [55, 0, 0] }],
    ["tail_plate", { rot: [48, 0, 0] }],
  ];
  for (let index = 1; index <= 7; index += 1) {
    curledPose.push(
      [`leg_${index}_l`, { rot: [0, 0, 68], scale: [0.24, 1, 1] }],
      [`leg_${index}_r`, { rot: [0, 0, -68], scale: [0.24, 1, 1] }],
    );
  }
  for (const side of ["l", "r"] as const) {
    curledPose.push(
      [`antenna_${side}`, { scale: [0.24, 0.24, 0.24] }],
      [`antenna_${side}_tip`, { scale: [0.24, 0.24, 0.24] }],
    );
  }

  clip("roll_up", {
    label: "Roll up",
    role: "action",
    fps: 24,
    loop: false,
    keys: transitionKeys(curledPose, 0, 0.24, 0.72),
  });
  clip("unroll", {
    label: "Unroll",
    role: "action",
    nextClip: "crawl",
    fps: 24,
    loop: false,
    keys: transitionKeys(curledPose, 1, 0.42, 0.72),
  });
  defaultClip("crawl");
});

function transitionKeys(
  curledPose: ReadonlyArray<readonly [string, TransformKey]>,
  curledAtStart: 0 | 1,
  middleTime: number,
  endTime: number,
): ClipKey[] {
  const keys: ClipKey[] = [];
  const startAmount = curledAtStart;
  const endAmount = 1 - curledAtStart;
  const middleAmount = startAmount + (endAmount - startAmount) * 0.58;
  for (const [partName, transform] of curledPose) {
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
