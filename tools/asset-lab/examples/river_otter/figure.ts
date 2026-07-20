import { figure } from "../../src/dsl";

// A box-only North American river otter with an elongated low torso, blunt
// whisker-colored muzzle, tiny ears, webbed dark paws, and a tapering two-stage
// tail. The swim cycle couples forepaw paddling to a lateral tail wave.
export default figure("river_otter", ({
  mat,
  asciiTexture,
  part,
  box,
  swim,
  swing,
}) => {
  mat("fur", "#5f402c");
  mat("fur_light", "#8a6548");
  mat("fur_dark", "#33271f");
  mat("cream", "#c5aa82");
  mat("muzzle", "#b99a73");
  mat("eye", "#171310");
  mat("paw", "#2b2823");

  asciiTexture("face", {
    palette: { ".": "#5f402c", "e": "#171310", "l": "#8a6548", "c": "#c5aa82" },
    pixels: [
      "ll....ll",
      ".ee..ee.",
      ".ee..ee.",
      "..cccc..",
      ".cccccc.",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#b99a73", "n": "#2c201a", "w": "#e3d5bb" },
    pixels: [
      "w......w",
      ".w.nn.w.",
      "..nnnn..",
      ".w.nn.w.",
      "w......w",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#5f402c", "l": "#8a6548", "d": "#33271f", "c": "#c5aa82" },
    pixels: [
      "dddddddddddddd",
      "dlllllllllllld",
      "l............l",
      "..............",
      "..cccccccccc..",
      ".cccccccccccc.",
    ],
  });

  part("body", box({
    at: [0, 0.65, 0.08],
    size: [0.64, 0.46, 1.5],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.28, -0.04],
    size: [0.5, 0.12, 1.16],
    material: "cream",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.04, -0.62],
    size: [0.68, 0.42, 0.42],
    material: "fur_light",
  }));
  part("head", box({
    parent: "shoulders",
    at: [0, 0.03, -0.42],
    size: [0.54, 0.42, 0.5],
    material: "fur_light",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.35],
    size: [0.4, 0.22, 0.24],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.22], ["r", 0.22]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.24, 0.07],
      size: [0.14, 0.16, 0.1],
      material: "fur_dark",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.24, -0.45],
    ["fr", 0.24, -0.45],
    ["bl", -0.24, 0.47],
    ["br", 0.24, 0.47],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.34, z],
      rot: [-8, 0, 0],
      size: [0.15, 0.3, 0.18],
      material: "fur_dark",
      joint: { pivot: [0, 0.15, 0], axis: [0, 0, 1] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.2, -0.06],
      size: [0.24, 0.09, 0.34],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, -0.03, 1.0],
    size: [0.36, 0.28, 0.72],
    material: "fur",
    joint: { pivot: [0, 0, -0.34], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.03, 0.58],
    size: [0.22, 0.18, 0.58],
    material: "fur_dark",
    joint: { pivot: [0, 0, -0.27], axis: [0, 1, 0] },
  }));

  swim("swim", {
    fps: 18,
    duration: 1.12,
    cycleDistance: 1.28,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.02,
    bodySwayDegrees: 3,
    finAxis: "z",
    finPhase: 0.2,
    finSwingDegrees: 12,
    leftFin: "leg_fl",
    rightFin: "leg_fr",
    tail: "tail",
    tailAxis: "y",
    tailSwingDegrees: 16,
    tailTip: "tail_tip",
    tailTipPhase: 0.11,
    tailTipSwingDegrees: 23,
    tracks: [
      swing("leg_bl", { axis: "z", degrees: 8, phase: 0.45 }),
      swing("leg_br", { axis: "z", degrees: -8, phase: 0.45 }),
    ],
  });
});
