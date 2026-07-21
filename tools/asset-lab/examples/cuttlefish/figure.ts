import { figure, type CycleTrack } from "../../src/dsl";

// A box-only common cuttlefish with a broad patterned mantle, traveling fin
// waves, large side eyes, eight short arms, and two feeding tentacles that can
// fire independently from its ordinary swimming clip.
export default figure("cuttlefish", ({
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
  mat("sepia", "#8d694d");
  mat("sepia_light", "#bd9671");
  mat("sepia_dark", "#57473d");
  mat("cream", "#d7c39d");
  mat("fin", "#9f7f5d");
  mat("iridescent", "#66877a");
  mat("eye", "#b9c46c");
  mat("pupil", "#171918");
  mat("sucker", "#e3b49f");

  asciiTexture("mantle_bands", {
    palette: { ".": "#8d694d", "l": "#bd9671", "d": "#57473d", "i": "#66877a" },
    pixels: [
      "dddddddddddddd",
      "dlliillliillid",
      "dl..ll..ll..ld",
      "d..ii....ii..d",
      "dll..llll..lld",
      "dillii..iillid",
      "dddddddddddddd",
    ],
  });
  asciiTexture("head_mask", {
    palette: { ".": "#8d694d", "l": "#bd9671", "d": "#57473d", "i": "#66877a" },
    pixels: [
      "ii......ii",
      "ill....lli",
      ".ld....dl.",
      "..dddddd..",
      ".ll....ll.",
      "..........",
    ],
  });
  asciiTexture("w_pupil", {
    palette: { ".": "#b9c46c", "p": "#171918", "i": "#66877a" },
    pixels: [
      "........",
      ".p....p.",
      "..p..p..",
      "...pp...",
      "..iiii..",
      "........",
    ],
  });
  asciiTexture("arm_suckers", {
    palette: { ".": "#bd9671", "s": "#e3b49f", "d": "#57473d" },
    pixels: ["d......d", ".s....s.", "..s..s..", ".s....s.", "d......d"],
  });

  part("mantle", box({
    at: [0, 0.92, 0.18],
    size: [1.12, 0.46, 1.36],
    material: "sepia",
    faces: {
      up: { texture: "mantle_bands" },
      east: { texture: "mantle_bands" },
      west: { texture: "mantle_bands" },
    },
  }));
  part("mantle_crown", box({
    parent: "mantle",
    at: [0, 0.27, 0.16],
    size: [0.86, 0.16, 1.0],
    material: "sepia_light",
    faces: { up: { texture: "mantle_bands" } },
  }));
  part("mantle_tip", box({
    parent: "mantle",
    at: [0, 0.02, 0.82],
    size: [0.72, 0.34, 0.36],
    material: "sepia_dark",
  }));
  part("head", box({
    parent: "mantle",
    at: [0, -0.03, -0.86],
    size: [0.82, 0.48, 0.52],
    material: "sepia_light",
    faces: { north: { texture: "head_mask" } },
    joint: { pivot: [0, 0, 0.23], axis: [1, 0, 0] },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.38, 0.08, -0.04],
      size: [0.2, 0.25, 0.3],
      material: "iridescent",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0.03, -0.17],
      size: [0.14, 0.14, 0.06],
      material: "eye",
      faces: { north: { texture: "w_pupil" } },
    }));
  }

  const finTracks: CycleTrack[] = [];
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    for (const [index, z, phase] of [
      [1, -0.42, 0],
      [2, 0, 0.11],
      [3, 0.42, 0.22],
    ] as const) {
      const name = `fin_${side}_${index}`;
      part(name, box({
        parent: "mantle",
        at: [sign * 0.65, -0.01, z],
        rot: [0, 0, sign * -4],
        size: [0.27, 0.075, 0.5],
        material: "fin",
        joint: { pivot: [sign * -0.12, 0, 0], axis: [0, 0, 1] },
      }));
      finTracks.push(swing(name, { axis: "z", degrees: sign * 9, phase }));
    }
  }

  const armTracks: CycleTrack[] = [];
  const armRoots: string[] = [];
  for (let index = 0; index < 8; index += 1) {
    const lane = index - 3.5;
    const x = lane * 0.095;
    const y = index % 2 === 0 ? -0.15 : -0.23;
    const yaw = lane * 4;
    const name = `arm_${index + 1}`;
    armRoots.push(`${name}_1`);
    part(`${name}_1`, box({
      parent: "head",
      at: [x, y, -0.43],
      rot: [index % 2 === 0 ? -6 : 4, yaw, lane * 1.5],
      size: [0.11, 0.1, 0.46],
      material: index % 2 === 0 ? "sepia" : "sepia_light",
      faces: { down: { texture: "arm_suckers" } },
      joint: { pivot: [0, 0, 0.2], axis: [0, 1, 0] },
    }));
    part(`${name}_tip`, box({
      parent: `${name}_1`,
      at: [lane * 0.012, 0, -0.36],
      rot: [index % 2 === 0 ? -7 : 7, yaw * 0.5, lane * 1.2],
      size: [0.08, 0.075, 0.3],
      material: "sepia_dark",
      faces: { down: { texture: "arm_suckers" } },
      joint: { pivot: [0, 0, 0.13], axis: [0, 1, 0] },
    }));
    armTracks.push(
      swing(`${name}_1`, { axis: "y", degrees: 5 + Math.abs(lane), phase: index * 0.045 }),
      swing(`${name}_tip`, { axis: "y", degrees: 8 + Math.abs(lane), phase: 0.12 + index * 0.045 }),
    );
  }

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`feeding_tentacle_${side}`, box({
      parent: "head",
      at: [sign * 0.16, -0.08, -0.42],
      size: [0.07, 0.07, 0.32],
      material: "cream",
      faces: { down: { texture: "arm_suckers" } },
      joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] },
    }));
    part(`feeding_club_${side}`, box({
      parent: `feeding_tentacle_${side}`,
      at: [0, 0, -0.24],
      size: [0.15, 0.1, 0.2],
      material: "sucker",
      faces: { down: { texture: "arm_suckers" } },
    }));
  }

  walkCycle("fin_swim", {
    label: "Fin-wave swim",
    role: "locomotion",
    fps: 24,
    duration: 1.16,
    loop: true,
    samples: 29,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.78,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("mantle", { axis: "y", amount: 0.022, phase: 0.5 }),
      swing("mantle", { axis: "y", degrees: 2.2, phase: 0 }),
      swing("head", { axis: "y", degrees: 2.8, phase: 0.5 }),
      ...finTracks,
      ...armTracks,
      swing("feeding_tentacle_l", { axis: "y", degrees: 4, phase: 0.18 }),
      swing("feeding_tentacle_r", { axis: "y", degrees: -4, phase: 0.18 }),
    ],
  });
  clip("feeding_strike", {
    label: "Feeding strike",
    role: "action",
    nextClip: "fin_swim",
    fps: 24,
    loop: false,
    keys: [
      ["feeding_tentacle_l", 0, { scale: [1, 1, 1] }],
      ["feeding_tentacle_l", 0.16, { scale: [1, 1, 1] }],
      ["feeding_tentacle_l", 0.24, { scale: [1, 1, 4.8] }],
      ["feeding_tentacle_l", 0.36, { scale: [1, 1, 4.8] }],
      ["feeding_tentacle_l", 0.52, { scale: [1, 1, 1] }],
      ["feeding_tentacle_l", 0.76, { scale: [1, 1, 1] }],
      ["feeding_tentacle_r", 0, { scale: [1, 1, 1] }],
      ["feeding_tentacle_r", 0.16, { scale: [1, 1, 1] }],
      ["feeding_tentacle_r", 0.24, { scale: [1, 1, 4.8] }],
      ["feeding_tentacle_r", 0.36, { scale: [1, 1, 4.8] }],
      ["feeding_tentacle_r", 0.52, { scale: [1, 1, 1] }],
      ["feeding_tentacle_r", 0.76, { scale: [1, 1, 1] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.16, { rot: [5, 0, 0] }],
      ["head", 0.24, { rot: [-7, 0, 0] }],
      ["head", 0.52, { rot: [3, 0, 0] }],
      ["head", 0.76, { rot: [0, 0, 0] }],
      ...armRoots.flatMap((name, index) => [
        [name, 0, { rot: [0, 0, 0] }],
        [name, 0.24, { rot: [0, (index - 3.5) * 2.5, (index - 3.5) * 1.6] }],
        [name, 0.52, { rot: [0, 0, 0] }],
        [name, 0.76, { rot: [0, 0, 0] }],
      ] as const),
    ],
  });
  defaultClip("fin_swim");
});
