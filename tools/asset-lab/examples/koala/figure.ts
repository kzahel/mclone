import { figure } from "../../src/dsl";

// A box-only adult koala in an upright clinging stance, with oversized stepped
// ears, a broad dark nose, pale belly, long inward-folded arms, and planted
// hind feet. The idle cycle breathes and scans without locomotion.
export default figure("koala", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  swing,
  bob,
  followThrough,
}) => {
  mat("fur", "#85898a");
  mat("fur_light", "#a8acad");
  mat("fur_dark", "#555b5d");
  mat("cream", "#d8d1bf");
  mat("ear_inner", "#7d6d6c");
  mat("nose", "#252729");
  mat("eye", "#171819");
  mat("paw", "#424749");

  asciiTexture("face", {
    palette: { ".": "#85898a", "l": "#a8acad", "e": "#171819", "d": "#555b5d" },
    pixels: [
      "ll....ll",
      ".ee..ee.",
      ".ee..ee.",
      "..dddd..",
      ".dddddd.",
      "........",
    ],
  });
  asciiTexture("belly_patch", {
    palette: { ".": "#d8d1bf", "l": "#eee8da", "g": "#a8acad" },
    pixels: [
      "gg......gg",
      "gllllllllg",
      ".llllllll.",
      ".llllllll.",
      "..llllll..",
      "...llll...",
    ],
  });
  asciiTexture("ear_fur", {
    palette: { ".": "#555b5d", "l": "#a8acad", "c": "#d8d1bf" },
    pixels: [
      "ccllllcc",
      "cl....lc",
      "l......l",
      "l......l",
      "cl....lc",
      "ccllllcc",
    ],
  });
  asciiTexture("paw_front", {
    palette: { ".": "#424749", "c": "#242729" },
    pixels: [
      "........",
      ".c.cc.c.",
      "cccccccc",
    ],
  });

  part("torso", box({
    at: [0, 1.0, 0.08],
    size: [0.66, 0.88, 0.5],
    material: "fur",
  }));
  part("belly", box({
    parent: "torso",
    at: [0, -0.03, -0.3],
    size: [0.5, 0.68, 0.11],
    material: "cream",
    faces: { north: { texture: "belly_patch" } },
  }));
  part("pelvis", box({
    parent: "torso",
    at: [0, -0.4, 0.12],
    size: [0.72, 0.42, 0.56],
    material: "fur_dark",
  }));
  part("shoulders", box({
    parent: "torso",
    at: [0, 0.27, -0.02],
    size: [0.76, 0.4, 0.5],
    material: "fur_light",
  }));
  part("head", box({
    parent: "torso",
    at: [0, 0.63, -0.06],
    size: [0.7, 0.58, 0.54],
    material: "fur",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.25, 0.12], axis: [0, 1, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.14, -0.37],
    size: [0.44, 0.24, 0.22],
    material: "fur_light",
  }));
  part("nose", box({
    parent: "muzzle",
    at: [0, 0.01, -0.15],
    size: [0.27, 0.18, 0.09],
    material: "nose",
  }));

  for (const [side, x] of [["l", -0.43], ["r", 0.43]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.08, 0.03],
      size: [0.32, 0.42, 0.14],
      material: "fur_dark",
      faces: { north: { texture: "ear_fur" } },
      joint: { pivot: [side === "l" ? 0.12 : -0.12, 0, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0, -0.09],
      size: [0.18, 0.28, 0.05],
      material: "ear_inner",
    }));
  }

  for (const [side, x, lean] of [
    ["l", -0.44, 16],
    ["r", 0.44, -16],
  ] as const) {
    part(`upper_arm_${side}`, box({
      parent: "shoulders",
      at: [x, -0.02, -0.05],
      rot: [-18, 0, lean],
      size: [0.22, 0.56, 0.22],
      material: "fur",
      joint: { pivot: [0, 0.27, 0], axis: [1, 0, 0] },
    }));
    part(`forearm_${side}`, box({
      parent: `upper_arm_${side}`,
      at: [side === "l" ? 0.08 : -0.08, -0.36, -0.13],
      rot: [-38, 0, side === "l" ? -18 : 18],
      size: [0.19, 0.44, 0.19],
      material: "fur_light",
    }));
    part(`hand_${side}`, box({
      parent: `forearm_${side}`,
      at: [0, -0.27, -0.09],
      size: [0.24, 0.16, 0.28],
      material: "paw",
      faces: { north: { texture: "paw_front" } },
    }));
  }

  for (const [side, x] of [["l", -0.22], ["r", 0.22]] as const) {
    part(`leg_${side}`, box({
      parent: "pelvis",
      at: [x, -0.32, 0.03],
      size: [0.2, 0.38, 0.22],
      material: "fur_dark",
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.24, -0.09],
      size: [0.3, 0.12, 0.4],
      material: "paw",
      faces: { north: { texture: "paw_front" } },
    }));
  }

  walkCycle("cling_idle", {
    fps: 24,
    duration: 2.2,
    loop: true,
    samples: 25,
    tracks: [
      bob("torso", { axis: "y", amount: 0.012, center: 0.012, phase: 0.25 }),
      swing("torso", { axis: "z", degrees: 1.2, phase: 0.25 }),
      swing("head", { axis: "y", degrees: 6, frequency: 0.5 }),
      swing("upper_arm_l", { axis: "x", degrees: 3, center: -18, phase: 0.5 }),
      swing("upper_arm_r", { axis: "x", degrees: 3, center: -18, phase: 0 }),
      followThrough("ear_l", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.13 }),
      followThrough("ear_r", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.13 }),
    ],
  });
});
