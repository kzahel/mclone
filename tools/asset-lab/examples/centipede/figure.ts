import {
  figure,
  type CycleTrack,
  type LocomotionContactSpec,
} from "../../src/dsl";

// A box-only giant centipede with eight articulated body plates, sixteen
// two-stage legs, long antennae, and paired venom claws. Small phased yaw
// offsets and a traveling footfall sequence send one wave down the whole body.
export default figure("centipede", ({
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
  mat("head", "#9a3828");
  mat("head_light", "#c85735");
  mat("segment", "#b64b2e");
  mat("segment_light", "#d16b3d");
  mat("segment_dark", "#783026");
  mat("leg", "#d08a45");
  mat("leg_tip", "#5f3425");
  mat("claw", "#643025");

  asciiTexture("segment_bands", {
    palette: { ".": "#b64b2e", "l": "#d16b3d", "d": "#783026", "s": "#e39452" },
    pixels: [
      "llllllll",
      "l......l",
      ".d.ss.d.",
      "d.ss.s.d",
      ".d.ss.d.",
      "dddddddd",
    ],
  });
  asciiTexture("head_face", {
    palette: { ".": "#9a3828", "l": "#c85735", "e": "#151715", "h": "#e39452" },
    pixels: [
      "ll....ll",
      ".ee..ee.",
      ".eh..he.",
      "..llll..",
      ".l....l.",
    ],
  });

  const segmentWidths = [0.46, 0.48, 0.5, 0.51, 0.5, 0.48, 0.44, 0.39] as const;
  for (let index = 1; index <= segmentWidths.length; index += 1) {
    const width = segmentWidths[index - 1]!;
    part(`segment_${index}`, box({
      ...(index === 1 ? {} : { parent: `segment_${index - 1}` }),
      at: index === 1 ? [0, 0.3, -0.7] : [0, -0.004, 0.24],
      size: [width, 0.2 - (index - 1) * 0.006, 0.26],
      material: index % 3 === 0 ? "segment_dark" : index % 2 === 0 ? "segment_light" : "segment",
      faces: {
        up: { texture: "segment_bands" },
        east: { texture: "segment_bands" },
        west: { texture: "segment_bands" },
      },
      ...(index === 1
        ? {}
        : { joint: { pivot: [0, 0, -0.12] as const, axis: [0, 1, 0] as const } }),
    }));
  }

  part("head", box({
    parent: "segment_1",
    at: [0, 0.02, -0.29],
    size: [0.52, 0.24, 0.32],
    material: "head",
    faces: { north: { texture: "head_face" } },
    joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] },
  }));
  part("tail_plate", box({
    parent: "segment_8",
    at: [0, -0.012, 0.25],
    size: [0.3, 0.14, 0.28],
    material: "segment_dark",
    faces: { up: { texture: "segment_bands" } },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`antenna_${side}`, box({
      parent: "head",
      at: [sign * 0.16, 0.06, -0.27],
      rot: [-6, sign * 14, 0],
      size: [0.035, 0.035, 0.36],
      material: "leg",
      joint: { pivot: [0, 0, 0.16], axis: [0, 1, 0] },
    }));
    part(`antenna_${side}_tip`, box({
      parent: `antenna_${side}`,
      at: [sign * 0.06, 0.01, -0.3],
      rot: [0, sign * 12, 0],
      size: [0.03, 0.03, 0.3],
      material: "leg_tip",
    }));
    part(`forcipule_${side}`, box({
      parent: "head",
      at: [sign * 0.17, -0.13, -0.2],
      rot: [8, sign * 18, 0],
      size: [0.09, 0.08, 0.28],
      material: "claw",
    }));
  }

  for (let index = 1; index <= segmentWidths.length; index += 1) {
    const width = segmentWidths[index - 1]!;
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${index}_${side}`, box({
        parent: `segment_${index}`,
        at: [sign * (width / 2 + 0.13), -0.07, 0],
        rot: [0, sign * (8 - index * 2), -sign * 12],
        size: [0.38, 0.05, 0.06],
        material: "leg",
        joint: { pivot: [sign * -0.18, 0, 0], axis: [0, 1, 0] },
      }));
      part(`leg_${index}_${side}_tip`, box({
        parent: `leg_${index}_${side}`,
        at: [sign * 0.32, -0.05, 0],
        rot: [0, 0, -sign * 10],
        size: [0.28, 0.042, 0.052],
        material: "leg_tip",
      }));
    }
  }

  const stanceRatio = 0.7;
  const contacts: LocomotionContactSpec[] = [];
  const tracks: CycleTrack[] = [
    bob("segment_1", { axis: "y", amount: 0.008, center: 0.008, phase: 0.5 }),
    swing("head", { axis: "y", degrees: 2.2, phase: 0.05 }),
    swing("antenna_l", { axis: "y", degrees: 7, phase: 0.1 }),
    swing("antenna_r", { axis: "y", degrees: -7, phase: 0.6 }),
  ];
  for (let index = 1; index <= segmentWidths.length; index += 1) {
    tracks.push(swing(`segment_${index}`, {
      axis: "y",
      degrees: 1.25 + index * 0.08,
      phase: (index - 1) * 0.09,
    }));
    const leftPhase = ((index - 1) * 0.11) % 1;
    for (const [side, phase] of [["l", leftPhase], ["r", (leftPhase + 0.5) % 1]] as const) {
      const tipName = `leg_${index}_${side}_tip`;
      contacts.push({
        part: tipName,
        phaseStart: phase,
        phaseEnd: (phase + stanceRatio) % 1,
        role: `segment-${index}-${side}`,
        stanceRatio,
      });
      tracks.push(contactSwing(`leg_${index}_${side}`, {
        axis: "y",
        degrees: 17,
        phase,
        stanceRatio,
      }));
    }
  }

  walkCycle("skitter", {
    label: "Traveling wave",
    role: "locomotion",
    fps: 22,
    duration: 1.12,
    loop: true,
    samples: 25,
    locomotion: {
      kind: "slither",
      cycleDistance: 0.52,
      direction: [0, 0, -1],
      units: "figure",
      contacts,
    },
    tracks,
  });
  defaultClip("skitter");
});
