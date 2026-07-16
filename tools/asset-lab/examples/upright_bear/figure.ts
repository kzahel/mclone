import { figure, type ClipKey } from "../../src/dsl";

interface WalkFrame {
  arm: number;
  bob: number;
  headYaw: number;
  leg: number;
  time: number;
}

const walkFrames: readonly WalkFrame[] = [
  { time: 0, leg: 18, arm: -12, bob: 0, headYaw: -1.5 },
  { time: 0.25, leg: 0, arm: 0, bob: 0.018, headYaw: 0 },
  { time: 0.5, leg: -18, arm: 12, bob: 0, headYaw: 1.5 },
  { time: 0.75, leg: 0, arm: 0, bob: 0.018, headYaw: 0 },
  { time: 1, leg: 18, arm: -12, bob: 0, headYaw: -1.5 },
];

const walkKeys: ClipKey[] = walkFrames.flatMap((frame): ClipKey[] => [
  ["leg_l", frame.time, { rot: [frame.leg, 0, 0] }],
  ["leg_r", frame.time, { rot: [-frame.leg, 0, 0] }],
  ["arm_l", frame.time, { rot: [frame.arm, 0, 0] }],
  ["arm_r", frame.time, { rot: [-frame.arm, 0, 0] }],
  ["torso", frame.time, { at: [0, frame.bob, 0] }],
  ["bear_head", frame.time, { rot: [0, frame.headYaw, 0] }],
]);

export default figure("upright_bear", ({ mat, asciiTexture, part, box, clip }) => {
  mat("fur", "#6b4a32");
  mat("fur_dark", "#3f2a1d");
  mat("muzzle", "#b98f65");
  mat("nose", "#17100c");
  mat("vest", "#2e6e62");
  mat("vest_dark", "#1d4842");
  mat("pants", "#303f4a");
  mat("boot", "#241c18");

  asciiTexture("face", {
    palette: {
      ".": "#6b4a32",
      "e": "#0f0b08",
      "m": "#b98f65",
      "n": "#17100c",
      "d": "#3f2a1d",
    },
    pixels: [
      "dddddddd",
      "d......d",
      ".ee..ee.",
      ".ee..ee.",
      "..mmmm..",
      "..mnnm..",
      "..mmmm..",
      "........",
    ],
  });

  part("torso", box({ at: [0, 0.78, 0], size: [0.62, 0.74, 0.34], material: "fur" }));
  part("vest_panel", box({
    parent: "torso",
    at: [0, 0.02, -0.01],
    size: [0.5, 0.64, 0.36],
    material: "vest",
  }));
  part("vest_band", box({
    parent: "torso",
    at: [0, -0.31, -0.012],
    size: [0.64, 0.08, 0.37],
    material: "vest_dark",
  }));
  part("bear_head", box({
    parent: "torso",
    at: [0, 0.63, 0],
    size: [0.54, 0.48, 0.5],
    material: "fur",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("snout", box({
    parent: "bear_head",
    at: [0, -0.06, -0.28],
    size: [0.28, 0.16, 0.14],
    material: "muzzle",
    faces: {
      north: { material: "nose" },
    },
  }));
  part("ear_l", box({
    parent: "bear_head",
    at: [-0.22, 0.28, 0.02],
    size: [0.16, 0.18, 0.14],
    material: "fur_dark",
  }));
  part("ear_r", box({
    parent: "bear_head",
    at: [0.22, 0.28, 0.02],
    size: [0.16, 0.18, 0.14],
    material: "fur_dark",
  }));

  part("arm_l", box({
    parent: "torso",
    at: [-0.47, 0.08, 0],
    size: [0.22, 0.66, 0.24],
    material: "fur",
    joint: { pivot: [0, 0.32, 0], axis: [1, 0, 0] },
  }));
  part("arm_r", box({
    parent: "torso",
    at: [0.47, 0.08, 0],
    size: [0.22, 0.66, 0.24],
    material: "fur",
    joint: { pivot: [0, 0.32, 0], axis: [1, 0, 0] },
  }));
  part("paw_l", box({
    parent: "arm_l",
    at: [0, -0.36, -0.01],
    size: [0.24, 0.13, 0.26],
    material: "fur_dark",
  }));
  part("paw_r", box({
    parent: "arm_r",
    at: [0, -0.36, -0.01],
    size: [0.24, 0.13, 0.26],
    material: "fur_dark",
  }));

  part("leg_l", box({
    parent: "torso",
    at: [-0.17, -0.7, 0],
    size: [0.22, 0.68, 0.24],
    material: "pants",
    joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] },
  }));
  part("leg_r", box({
    parent: "torso",
    at: [0.17, -0.7, 0],
    size: [0.22, 0.68, 0.24],
    material: "pants",
    joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] },
  }));
  part("foot_l", box({
    parent: "leg_l",
    at: [0, -0.38, -0.05],
    size: [0.25, 0.11, 0.32],
    material: "boot",
  }));
  part("foot_r", box({
    parent: "leg_r",
    at: [0, -0.38, -0.05],
    size: [0.25, 0.11, 0.32],
    material: "boot",
  }));

  clip("walk", {
    loop: true,
    fps: 8,
    locomotion: {
      kind: "biped-walk",
      cycleDistance: 0.74,
      speed: 0.74,
      units: "figure",
      direction: [0, 0, 1],
      contacts: [
        {
          part: "foot_l",
          phaseStart: 0,
          phaseEnd: 0.66,
          role: "left",
          stanceRatio: 0.66,
        },
        {
          part: "foot_r",
          phaseStart: 0.5,
          phaseEnd: 0.16,
          role: "right",
          stanceRatio: 0.66,
        },
      ],
    },
    keys: walkKeys,
  });
});
