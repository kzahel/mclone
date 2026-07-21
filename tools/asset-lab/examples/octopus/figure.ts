import { figure, type CycleTrack } from "../../src/dsl";

// A box-only common octopus with a tall mottled mantle, broad expressive head,
// raised eyes, and eight independently rooted three-stage tentacles. The swim
// gathers the arms into a coordinated trailing wave rather than treating them
// as terrestrial legs.
export default figure("octopus", ({
  asciiTexture,
  bob,
  box,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("skin", "#a65358");
  mat("skin_light", "#ca7470");
  mat("skin_dark", "#713c4a");
  mat("mantle", "#8f4857");
  mat("sucker", "#e4b5a0");
  mat("eye", "#d8bc62");
  mat("pupil", "#191514");
  mat("mouth", "#432632");

  asciiTexture("mantle_mottle", {
    palette: { ".": "#8f4857", "l": "#ca7470", "d": "#713c4a", "s": "#a65358" },
    pixels: [
      "dddddddddd",
      "dll..ll..d",
      "d..ss..l.d",
      "dl..d....d",
      "d..l..ss.d",
      "d.ss..l..d",
      "dddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#a65358", "l": "#ca7470", "d": "#713c4a" },
    pixels: [
      "ll......ll",
      "..........",
      "..d....d..",
      "...dddd...",
      "..........",
    ],
  });
  asciiTexture("sucker_rows", {
    palette: { ".": "#a65358", "s": "#e4b5a0", "d": "#713c4a" },
    pixels: [
      "d......d",
      ".s....s.",
      "..s..s..",
      ".s....s.",
      "..s..s..",
      ".s....s.",
      "d......d",
    ],
  });

  part("head", box({
    at: [0, 1.02, -0.16],
    size: [0.82, 0.5, 0.72],
    material: "skin",
    faces: { north: { texture: "face" } },
  }));
  part("mantle", box({
    parent: "head",
    at: [0, 0.46, 0.2],
    size: [0.72, 0.68, 0.7],
    material: "mantle",
    faces: {
      up: { texture: "mantle_mottle" },
      east: { texture: "mantle_mottle" },
      west: { texture: "mantle_mottle" },
    },
    joint: { pivot: [0, -0.3, -0.18], axis: [0, 1, 0] },
  }));
  part("mantle_crown", box({
    parent: "mantle",
    at: [0, 0.38, 0.02],
    size: [0.56, 0.22, 0.52],
    material: "skin_light",
    faces: { up: { texture: "mantle_mottle" } },
  }));
  part("mouth", box({
    parent: "head",
    at: [0, -0.3, -0.18],
    size: [0.2, 0.12, 0.18],
    material: "mouth",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.31, 0.12, -0.26],
      size: [0.24, 0.2, 0.24],
      material: "skin_dark",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0.01, -0.14],
      size: [0.13, 0.12, 0.06],
      material: "eye",
      faces: { north: { material: "pupil" } },
    }));
  }

  const tentacleTracks: CycleTrack[] = [];
  for (let index = 0; index < 8; index += 1) {
    const angle = (index / 8) * Math.PI * 2;
    const xDirection = Math.sin(angle);
    const zDirection = Math.cos(angle);
    const x = xDirection * 0.34;
    const z = zDirection * 0.28;
    const phase = (index % 4) * 0.035;
    const name = `tentacle_${index + 1}`;

    part(`${name}_1`, box({
      parent: "head",
      at: [x, -0.43, z],
      rot: [zDirection * 10, 0, -xDirection * 12],
      size: [0.17, 0.5, 0.18],
      material: index % 2 === 0 ? "skin_dark" : "skin",
      faces: {
        east: { texture: "sucker_rows" },
        west: { texture: "sucker_rows" },
      },
      joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] },
    }));
    part(`${name}_2`, box({
      parent: `${name}_1`,
      at: [xDirection * 0.06, -0.42, zDirection * 0.07],
      rot: [zDirection * 7, 0, -xDirection * 8],
      size: [0.13, 0.42, 0.14],
      material: "skin_light",
      faces: {
        east: { texture: "sucker_rows" },
        west: { texture: "sucker_rows" },
      },
      joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] },
    }));
    part(`${name}_tip`, box({
      parent: `${name}_2`,
      at: [xDirection * 0.05, -0.34, zDirection * 0.06],
      rot: [zDirection * 6, 0, -xDirection * 7],
      size: [0.09, 0.32, 0.1],
      material: "skin_dark",
      faces: {
        east: { texture: "sucker_rows" },
        west: { texture: "sucker_rows" },
      },
      joint: { pivot: [0, 0.14, 0], axis: [1, 0, 0] },
    }));

    tentacleTracks.push(
      swing(`${name}_1`, { axis: "x", degrees: zDirection * 14, phase }),
      swing(`${name}_1`, { axis: "z", degrees: -xDirection * 14, phase }),
      swing(`${name}_2`, { axis: "x", degrees: zDirection * 20, phase: phase + 0.1 }),
      swing(`${name}_2`, { axis: "z", degrees: -xDirection * 20, phase: phase + 0.1 }),
      swing(`${name}_tip`, { axis: "x", degrees: zDirection * 27, phase: phase + 0.2 }),
      swing(`${name}_tip`, { axis: "z", degrees: -xDirection * 27, phase: phase + 0.2 }),
    );
  }

  walkCycle("jet_swim", {
    label: "Jet swim",
    role: "locomotion",
    fps: 24,
    duration: 1.12,
    loop: true,
    samples: 33,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.92,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("head", { axis: "y", amount: 0.025, phase: 0.5 }),
      swing("head", { axis: "y", degrees: 2.4, phase: 0.25 }),
      bob("mantle", { axis: "z", amount: 0.035, phase: 0.5 }),
      swing("mantle", { axis: "y", degrees: 2, phase: 0.75 }),
      ...tentacleTracks,
    ],
  });
  defaultClip("jet_swim");
});
