import { figure, type CycleTrack } from "../../src/dsl";

// A box-only ochre sea star with five independently rooted, three-stage arms.
// Its low creep is a restrained traveling wave rather than a legged walk.
export default figure("starfish", ({
  asciiTexture,
  bob,
  box,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("disc", "#bd6640");
  mat("disc_light", "#dc8a54");
  mat("arm", "#c87345");
  mat("arm_light", "#e19a5f");
  mat("arm_dark", "#84452f");
  mat("tube_feet", "#e7bd83");

  asciiTexture("disc_spots", {
    palette: { ".": "#bd6640", "l": "#dc8a54", "d": "#84452f" },
    pixels: [
      "ddlllllldd",
      "dl..d...ld",
      "l.d...d..l",
      "l...ll...l",
      "ld.....d.l",
      "l..d...d.l",
      "dl...d..ld",
      "ddlllllldd",
    ],
  });
  asciiTexture("arm_spots", {
    palette: { ".": "#c87345", "l": "#e19a5f", "d": "#84452f" },
    pixels: [
      "dddddd",
      "l....l",
      "..d...",
      ".l..d.",
      "...l..",
      ".d....",
      "l....l",
      "dddddd",
    ],
  });
  asciiTexture("tube_rows", {
    palette: { ".": "#e7bd83", "d": "#84452f" },
    pixels: [
      "d....d",
      ".d..d.",
      "d....d",
      ".d..d.",
      "d....d",
      ".d..d.",
    ],
  });

  part("disc", box({
    at: [0, 0.14, 0],
    size: [0.68, 0.2, 0.68],
    material: "disc",
    faces: { up: { texture: "disc_spots" } },
  }));
  part("disc_crown", box({
    parent: "disc",
    at: [0, 0.13, 0],
    size: [0.34, 0.1, 0.34],
    material: "disc_light",
    faces: { up: { texture: "disc_spots" } },
  }));

  const tracks: CycleTrack[] = [
    bob("disc", { axis: "y", amount: 0.012, center: 0.012, phase: 0.5 }),
    swing("disc", { axis: "y", degrees: 1.5, phase: 0.25 }),
  ];
  for (let index = 0; index < 5; index += 1) {
    const angle = index * Math.PI * 2 / 5;
    const yaw = index * 72;
    const x = Math.sin(angle) * 0.43;
    const z = Math.cos(angle) * 0.43;
    const name = `arm_${index + 1}`;

    part(`${name}_root`, box({
      parent: "disc",
      at: [x, -0.04, z],
      rot: [0, yaw, 0],
      size: [0.3, 0.15, 0.56],
      material: index % 2 === 0 ? "arm" : "arm_light",
      faces: {
        up: { texture: "arm_spots" },
        down: { texture: "tube_rows" },
      },
      joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] },
    }));
    part(`${name}_mid`, box({
      parent: `${name}_root`,
      at: [0, -0.01, 0.43],
      size: [0.22, 0.13, 0.46],
      material: "arm",
      faces: {
        up: { texture: "arm_spots" },
        down: { texture: "tube_rows" },
      },
      joint: { pivot: [0, 0, -0.2], axis: [0, 1, 0] },
    }));
    part(`${name}_tip`, box({
      parent: `${name}_mid`,
      at: [0, -0.01, 0.35],
      size: [0.14, 0.1, 0.34],
      material: "arm_dark",
      faces: {
        up: { texture: "arm_spots" },
        down: { texture: "tube_rows" },
      },
      joint: { pivot: [0, 0, -0.14], axis: [0, 1, 0] },
    }));

    const phase = index * 0.18;
    tracks.push(
      swing(`${name}_root`, { axis: "y", degrees: 2.5, phase }),
      swing(`${name}_mid`, { axis: "y", degrees: 5, phase: phase + 0.1 }),
      swing(`${name}_tip`, { axis: "y", degrees: 8, phase: phase + 0.2 }),
      swing(`${name}_mid`, { axis: "x", degrees: 1.8, phase: phase + 0.25 }),
      swing(`${name}_tip`, { axis: "x", degrees: 3, phase: phase + 0.35 }),
    );
  }

  walkCycle("creep", {
    label: "Tube-foot creep",
    role: "locomotion",
    fps: 20,
    duration: 2.4,
    loop: true,
    samples: 41,
    locomotion: {
      kind: "slither",
      cycleDistance: 0.16,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks,
  });
  defaultClip("creep");
});
