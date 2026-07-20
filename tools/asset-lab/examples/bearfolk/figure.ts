import { figure } from "../../src/dsl";

// A bear character on the same cuboid biped grammar as the canonical player.
// Species identity lives in the broad head, muzzle, small ears, paws, and vest.
export default figure("bearfolk", ({
  mat,
  asciiTexture,
  part,
  box,
  bipedWalk,
}) => {
  mat("fur", "#6d4a32");
  mat("fur_dark", "#3c281d");
  mat("fur_light", "#b28663");
  mat("cloth", "#436b57");
  mat("claw", "#1b1512");

  asciiTexture("face", {
    palette: { ".": "#6d4a32", "e": "#11100e", "m": "#b28663", "n": "#1b1512" },
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
  asciiTexture("paw_face", {
    palette: { "p": "#3c281d", "c": "#1b1512" },
    pixels: [
      "pppppp",
      "pcpccp",
      "pppppp",
    ],
  });

  part("torso", box({
    at: [0, 0.82, 0],
    size: [0.64, 0.78, 0.34],
    material: "fur",
  }));
  part("vest", box({
    parent: "torso",
    at: [0, 0.02, -0.09],
    size: [0.68, 0.58, 0.2],
    material: "cloth",
  }));
  part("belly", box({
    parent: "torso",
    at: [0, -0.12, -0.2],
    size: [0.42, 0.36, 0.08],
    material: "fur_light",
  }));
  part("head", box({
    parent: "torso",
    at: [0, 0.65, -0.01],
    size: [0.56, 0.52, 0.5],
    material: "fur",
    joint: { pivot: [0, -0.22, 0], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.06, -0.31],
    size: [0.36, 0.25, 0.16],
    material: "fur_light",
    faces: { north: { texture: "face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.23, 0.29, -0.01],
    size: [0.18, 0.18, 0.12],
    material: "fur_dark",
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.23, 0.29, -0.01],
    size: [0.18, 0.18, 0.12],
    material: "fur_dark",
  }));

  for (const [side, x] of [["l", -0.44], ["r", 0.44]] as const) {
    part(`arm_${side}`, box({
      parent: "torso",
      at: [x, 0.08, 0],
      size: [0.22, 0.7, 0.24],
      material: "fur",
      joint: { pivot: [0, 0.35, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${side}`, box({
      parent: `arm_${side}`,
      at: [0, -0.4, -0.03],
      size: [0.26, 0.16, 0.28],
      material: "fur_dark",
      faces: { north: { texture: "paw_face" } },
    }));
  }

  for (const [side, x] of [["l", -0.18], ["r", 0.18]] as const) {
    part(`leg_${side}`, box({
      parent: "torso",
      at: [x, -0.67, 0],
      size: [0.24, 0.64, 0.26],
      material: "fur_dark",
      joint: { pivot: [0, 0.32, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.38, -0.07],
      size: [0.3, 0.12, 0.38],
      material: "fur_dark",
      faces: { north: { texture: "paw_face" } },
    }));
  }

  bipedWalk("walk", {
    fps: 12,
    duration: 1.1,
    cycleDistance: 0.74,
    loop: true,
    samples: 11,
    armSwingDegrees: 12,
    body: "torso",
    bodyBob: 0.013,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 1.5,
    leftArm: "arm_l",
    leftContact: "foot_l",
    leftLeg: "leg_l",
    rightArm: "arm_r",
    rightContact: "foot_r",
    rightLeg: "leg_r",
    stanceRatio: 0.66,
    swingDegrees: 18,
  });
});
