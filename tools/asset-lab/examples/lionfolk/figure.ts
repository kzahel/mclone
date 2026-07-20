import { figure } from "../../src/dsl";

// A lion character on the canonical player biped grammar. A cuboid mane frame
// and articulated tail add the animal silhouette without rounding the limbs.
export default figure("lionfolk", ({
  mat,
  asciiTexture,
  part,
  box,
  bipedWalk,
  swing,
}) => {
  mat("fur", "#c88a3d");
  mat("fur_light", "#e0b26c");
  mat("mane", "#6d3d1f");
  mat("mane_dark", "#3d2318");
  mat("cloth", "#7c3f45");
  mat("paw", "#4a2c1d");

  asciiTexture("face", {
    palette: { ".": "#c88a3d", "e": "#14100d", "m": "#e0b26c", "n": "#4a2c1d" },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "..mmmm..",
      "..mnmm..",
      "..mmmm..",
      "........",
      "........",
    ],
  });

  part("torso", box({
    at: [0, 0.82, 0],
    size: [0.58, 0.76, 0.32],
    material: "fur",
  }));
  part("tunic", box({
    parent: "torso",
    at: [0, -0.04, -0.03],
    size: [0.62, 0.5, 0.34],
    material: "cloth",
  }));
  part("head", box({
    parent: "torso",
    at: [0, 0.64, -0.01],
    size: [0.5, 0.48, 0.46],
    material: "fur",
    joint: { pivot: [0, -0.21, 0], axis: [1, 0, 0] },
  }));
  part("mane_back", box({
    parent: "head",
    at: [0, 0, 0.16],
    size: [0.68, 0.64, 0.18],
    material: "mane",
  }));
  part("mane_top", box({
    parent: "head",
    at: [0, 0.3, -0.01],
    size: [0.62, 0.16, 0.42],
    material: "mane_dark",
  }));
  part("mane_l", box({
    parent: "head",
    at: [-0.32, 0, 0],
    size: [0.15, 0.5, 0.4],
    material: "mane",
  }));
  part("mane_r", box({
    parent: "head",
    at: [0.32, 0, 0],
    size: [0.15, 0.5, 0.4],
    material: "mane",
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.06, -0.3],
    size: [0.34, 0.21, 0.16],
    material: "fur_light",
    faces: { north: { texture: "face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.2, 0.26, -0.02],
    size: [0.14, 0.14, 0.11],
    material: "mane_dark",
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.2, 0.26, -0.02],
    size: [0.14, 0.14, 0.11],
    material: "mane_dark",
  }));

  for (const [side, x] of [["l", -0.41], ["r", 0.41]] as const) {
    part(`arm_${side}`, box({
      parent: "torso",
      at: [x, 0.1, 0],
      size: [0.19, 0.66, 0.21],
      material: "fur",
      joint: { pivot: [0, 0.33, 0], axis: [1, 0, 0] },
    }));
    part(`hand_${side}`, box({
      parent: `arm_${side}`,
      at: [0, -0.38, -0.02],
      size: [0.22, 0.14, 0.25],
      material: "paw",
    }));
  }

  for (const [side, x] of [["l", -0.17], ["r", 0.17]] as const) {
    part(`leg_${side}`, box({
      parent: "torso",
      at: [x, -0.66, 0],
      size: [0.21, 0.68, 0.23],
      material: "fur",
      joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.4, -0.06],
      size: [0.24, 0.1, 0.34],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "torso",
    at: [0, -0.18, 0.25],
    rot: [-35, 0, 0],
    size: [0.09, 0.58, 0.09],
    material: "fur",
    joint: { pivot: [0, 0.29, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.36, 0],
    size: [0.2, 0.2, 0.18],
    material: "mane",
  }));

  bipedWalk("walk", {
    fps: 12,
    duration: 0.88,
    cycleDistance: 0.84,
    loop: true,
    samples: 11,
    armSwingDegrees: 16,
    body: "torso",
    bodyBob: 0.016,
    bodyBobCenter: 0.016,
    head: "head",
    headSwingDegrees: 2,
    leftArm: "arm_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "arm_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.62,
    swingDegrees: 22,
    tracks: [
      swing("tail", { axis: "z", degrees: 9, phase: 0.5 }),
    ],
  });
});
