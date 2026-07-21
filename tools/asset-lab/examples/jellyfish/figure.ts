import { figure, type ClipKey } from "../../src/dsl";

// A box-only moon jellyfish with a stepped bell, four broad oral arms, and
// eight paired tentacle chains. The bell contracts independently of the arms
// so the pulse reads as propulsion instead of scaling the entire figure.
export default figure("jellyfish", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  mat,
  part,
}) => {
  mat("bell", "#668fc2");
  mat("bell_light", "#9ec7df");
  mat("bell_dark", "#405e91");
  mat("rim", "#557aae");
  mat("oral", "#8bafd0");
  mat("tentacle", "#b0cfe0");
  mat("tentacle_tip", "#7398bd");

  asciiTexture("bell_cells", {
    palette: { ".": "#668fc2", "l": "#9ec7df", "d": "#405e91" },
    pixels: [
      "ddlllllldd",
      "dll....lld",
      "l..d..d..l",
      "l........l",
      "l.d....d.l",
      "l........l",
      "l..d..d..l",
      "dll....lld",
      "ddlllllldd",
    ],
  });
  asciiTexture("rim_bands", {
    palette: { ".": "#557aae", "l": "#9ec7df", "d": "#405e91" },
    pixels: [
      "llllllllll",
      "..dd..dd..",
      "dddddddddd",
      "..ll..ll..",
    ],
  });
  asciiTexture("arm_bands", {
    palette: { ".": "#8bafd0", "l": "#b0cfe0", "d": "#557aae" },
    pixels: [
      "lldd",
      "....",
      "ddll",
      "....",
      "lldd",
      "....",
      "ddll",
      "....",
    ],
  });

  part("core", box({
    at: [0, 1.34, 0],
    size: [0.12, 0.12, 0.12],
    material: "bell_dark",
  }));
  part("bell_lower", box({
    parent: "core",
    at: [0, 0, 0],
    size: [1.18, 0.42, 1.08],
    material: "bell",
    faces: {
      up: { texture: "bell_cells" },
      north: { texture: "bell_cells" },
      south: { texture: "bell_cells" },
    },
  }));
  part("bell_crown", box({
    parent: "core",
    at: [0, 0.28, 0],
    size: [0.9, 0.3, 0.82],
    material: "bell_light",
    faces: { up: { texture: "bell_cells" } },
  }));
  part("bell_apex", box({
    parent: "core",
    at: [0, 0.49, 0],
    size: [0.48, 0.18, 0.44],
    material: "bell_light",
  }));

  for (const [name, at, size] of [
    ["front", [0, -0.25, -0.43], [0.86, 0.16, 0.22]],
    ["back", [0, -0.25, 0.43], [0.86, 0.16, 0.22]],
    ["left", [-0.48, -0.25, 0], [0.22, 0.16, 0.66]],
    ["right", [0.48, -0.25, 0], [0.22, 0.16, 0.66]],
  ] as const) {
    part(`rim_${name}`, box({
      parent: "core",
      at,
      size,
      material: "rim",
      faces: {
        north: { texture: "rim_bands" },
        south: { texture: "rim_bands" },
        east: { texture: "rim_bands" },
        west: { texture: "rim_bands" },
      },
    }));
  }

  const oralArmPositions = [
    [-0.23, -0.23],
    [0.23, -0.23],
    [-0.23, 0.23],
    [0.23, 0.23],
  ] as const;
  for (const [index, [x, z]] of oralArmPositions.entries()) {
    const name = `oral_arm_${index + 1}`;
    part(name, box({
      parent: "core",
      at: [x, -0.43, z],
      rot: [index < 2 ? -5 : 5, 0, x < 0 ? -5 : 5],
      size: [0.22, 0.68, 0.22],
      material: "oral",
      faces: { north: { texture: "arm_bands" }, south: { texture: "arm_bands" } },
      joint: { pivot: [0, 0.3, 0], axis: [1, 0, 0] },
    }));
    part(`${name}_tip`, box({
      parent: name,
      at: [0, -0.51, 0],
      rot: [index < 2 ? 7 : -7, 0, x < 0 ? 7 : -7],
      size: [0.17, 0.48, 0.17],
      material: "oral",
      faces: { north: { texture: "arm_bands" }, south: { texture: "arm_bands" } },
      joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
    }));
  }

  for (let index = 0; index < 8; index += 1) {
    const angle = index * Math.PI / 4;
    const x = Math.sin(angle) * 0.48;
    const z = Math.cos(angle) * 0.43;
    const name = `tentacle_${index + 1}`;
    part(name, box({
      parent: "core",
      at: [x, -0.46, z],
      rot: [Math.cos(angle) * 5, 0, Math.sin(angle) * -5],
      size: [0.065, 0.76, 0.065],
      material: "tentacle",
      joint: { pivot: [0, 0.35, 0], axis: [1, 0, 0] },
    }));
    part(`${name}_tip`, box({
      parent: name,
      at: [0, -0.57, 0],
      rot: [Math.cos(angle) * -8, 0, Math.sin(angle) * 8],
      size: [0.05, 0.42, 0.05],
      material: "tentacle_tip",
      joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] },
    }));
  }

  clip("pulse", {
    label: "Bell pulse",
    role: "locomotion",
    fps: 24,
    loop: true,
    locomotion: {
      kind: "swim",
      cycleDistance: 0.36,
      direction: [0, 1, 0],
      units: "figure",
    },
    keys: pulseKeys(1.28, 33),
  });
  defaultClip("pulse");
});

function pulseKeys(duration: number, samples: number): ClipKey[] {
  const keys: ClipKey[] = [];
  const bellParts = [
    "bell_lower",
    "bell_crown",
    "bell_apex",
    "rim_front",
    "rim_back",
    "rim_left",
    "rim_right",
  ];
  for (let sample = 0; sample < samples; sample += 1) {
    const phase = sample / (samples - 1);
    const time = duration * phase;
    const pulse = asymmetricPulse(phase);
    const surge = Math.sin(phase * Math.PI * 2 - 0.25) * 0.045;
    keys.push(["core", time, { at: [0, surge, 0] }]);

    for (const partName of bellParts) {
      keys.push([partName, time, {
        scale: [1 - pulse * 0.16, 1 + pulse * 0.18, 1 - pulse * 0.16],
      }]);
    }
    keys.push(
      ["bell_crown", time, { at: [0, pulse * 0.035, 0], scale: [1 - pulse * 0.16, 1 + pulse * 0.18, 1 - pulse * 0.16] }],
      ["bell_apex", time, { at: [0, pulse * 0.075, 0], scale: [1 - pulse * 0.16, 1 + pulse * 0.18, 1 - pulse * 0.16] }],
    );

    for (let index = 0; index < 4; index += 1) {
      const wave = Math.sin(phase * Math.PI * 2 + index * 1.1);
      const sign = index % 2 === 0 ? -1 : 1;
      keys.push(
        [`oral_arm_${index + 1}`, time, { rot: [wave * 4 - pulse * 6, 0, sign * pulse * 5] }],
        [`oral_arm_${index + 1}_tip`, time, { rot: [wave * -8 - pulse * 9, 0, sign * pulse * -7] }],
      );
    }
    for (let index = 0; index < 8; index += 1) {
      const angle = index * Math.PI / 4;
      const wave = Math.sin(phase * Math.PI * 2 + index * 0.68);
      keys.push(
        [`tentacle_${index + 1}`, time, {
          rot: [Math.cos(angle) * (wave * 5 - pulse * 7), 0, Math.sin(angle) * (wave * -5 + pulse * 7)],
        }],
        [`tentacle_${index + 1}_tip`, time, {
          rot: [Math.cos(angle) * (wave * -10 - pulse * 8), 0, Math.sin(angle) * (wave * 10 + pulse * 8)],
        }],
      );
    }
  }
  return keys;
}

function asymmetricPulse(phase: number): number {
  if (phase <= 0.22) {
    return smoothstep(phase / 0.22);
  }
  return 1 - smoothstep((phase - 0.22) / 0.78);
}

function smoothstep(value: number): number {
  return value * value * (3 - 2 * value);
}
