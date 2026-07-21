import { figure } from "../../src/dsl";

// A box-only adult Komodo dragon with a heavy scaled torso, muscular neck,
// blunt monitor head, visible forked tongue, four stout sprawled limbs, broad
// clawed feet, and a four-stage tapering tail.
export default figure("komodo_dragon", ({
  asciiTexture,
  box,
  followThrough,
  mat,
  part,
  quadrupedWalk,
  swing,
}) => {
  mat("hide", "#625f43");
  mat("hide_light", "#817c57");
  mat("hide_dark", "#3e402f");
  mat("scale", "#4c4d36");
  mat("belly", "#a19970");
  mat("eye", "#d6ad3b");
  mat("pupil", "#15160f");
  mat("mouth", "#4e2a2a");
  mat("tongue", "#9f5363");
  mat("claw", "#d2c49a");

  asciiTexture("body_scales", {
    palette: { ".": "#625f43", "l": "#817c57", "d": "#3e402f", "s": "#4c4d36" },
    pixels: [
      "dddddddddddddd",
      "dlss..ss..ssld",
      "d..l....l....d",
      "dl...ss...ssld",
      "d..s....s....d",
      "dl....ll....ld",
      "dddddddddddddd",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#817c57", "n": "#292c21", "m": "#4e2a2a" },
    pixels: [
      ".nn......nn.",
      "............",
      "............",
      ".mmmmmmmmmm.",
      "..mmmmmmmm..",
    ],
  });
  asciiTexture("foot_claws", {
    palette: { ".": "#3e402f", "c": "#d2c49a" },
    pixels: [
      "..........",
      ".c.c.c.c..",
      "cccccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.58, 0.12],
    size: [0.78, 0.48, 1.44],
    material: "hide",
    faces: {
      up: { texture: "body_scales" },
      east: { texture: "body_scales" },
      west: { texture: "body_scales" },
    },
  }));
  part("back_ridge", box({
    parent: "body",
    at: [0, 0.3, 0.08],
    size: [0.52, 0.13, 1.0],
    material: "scale",
    faces: { up: { texture: "body_scales" } },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.29, -0.02],
    size: [0.64, 0.12, 1.12],
    material: "belly",
  }));

  part("neck_1", box({
    parent: "body",
    at: [0, 0.03, -0.85],
    size: [0.66, 0.42, 0.4],
    material: "hide_dark",
    joint: { pivot: [0, 0, 0.18], axis: [0, 1, 0] },
  }));
  part("neck_2", box({
    parent: "neck_1",
    at: [0, 0.02, -0.34],
    size: [0.61, 0.4, 0.38],
    material: "hide_light",
    joint: { pivot: [0, 0, 0.17], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck_2",
    at: [0, 0.02, -0.37],
    size: [0.58, 0.38, 0.46],
    material: "hide_light",
    faces: {
      up: { texture: "body_scales" },
      east: { texture: "body_scales" },
      west: { texture: "body_scales" },
    },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.05, -0.38],
    size: [0.5, 0.27, 0.36],
    material: "hide_light",
    faces: { north: { texture: "snout_face" } },
  }));
  part("lower_jaw", box({
    parent: "head",
    at: [0, -0.23, -0.23],
    size: [0.48, 0.1, 0.6],
    material: "mouth",
  }));
  part("tongue_root", box({
    parent: "snout",
    at: [0, -0.07, -0.32],
    size: [0.06, 0.035, 0.42],
    material: "tongue",
    joint: { pivot: [0, 0, 0.19], axis: [1, 0, 0] },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`tongue_${side}`, box({
      parent: "tongue_root",
      at: [sign * 0.045, 0, -0.27],
      rot: [0, sign * 14, 0],
      size: [0.035, 0.025, 0.22],
      material: "tongue",
    }));
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.22, 0.21, -0.08],
      size: [0.17, 0.14, 0.18],
      material: "hide_dark",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0.01, -0.105],
      size: [0.09, 0.08, 0.04],
      material: "eye",
      faces: { north: { material: "pupil" } },
    }));
  }

  for (const [row, z, bend] of [["front", -0.48, -0.08], ["rear", 0.48, 0.1]] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`upper_leg_${row}_${side}`, box({
        parent: "body",
        at: [sign * 0.49, -0.2, z],
        rot: [0, sign * (row === "front" ? 8 : -8), -sign * 10],
        size: [0.44, 0.16, 0.24],
        material: "hide_dark",
        joint: { pivot: [sign * -0.2, 0.04, 0], axis: [1, 0, 0] },
      }));
      part(`foreleg_${row}_${side}`, box({
        parent: `upper_leg_${row}_${side}`,
        at: [sign * 0.32, -0.12, bend],
        rot: [0, 0, -sign * 12],
        size: [0.34, 0.13, 0.19],
        material: "hide_light",
      }));
      part(`foot_${row}_${side}`, box({
        parent: `foreleg_${row}_${side}`,
        at: [sign * 0.23, -0.1, -0.08],
        size: [0.3, 0.09, 0.4],
        material: "hide_dark",
        faces: { north: { texture: "foot_claws" } },
      }));
    }
  }

  const tailWidths = [0.58, 0.43, 0.29, 0.16] as const;
  const tailLengths = [0.7, 0.68, 0.62, 0.54] as const;
  for (let index = 1; index <= tailWidths.length; index += 1) {
    part(`tail_${index}`, box({
      ...(index === 1 ? { parent: "body" } : { parent: `tail_${index - 1}` }),
      at: index === 1 ? [0, 0.02, 0.93] : [0, -0.02, tailLengths[index - 2]! * 0.84],
      size: [tailWidths[index - 1]!, 0.36 - index * 0.055, tailLengths[index - 1]!],
      material: index % 2 === 0 ? "hide_light" : "hide",
      ...(index <= 2 ? { faces: { up: { texture: "body_scales" } } } : {}),
      joint: { pivot: [0, 0, -tailLengths[index - 1]! * 0.46], axis: [0, 1, 0] },
    }));
  }

  quadrupedWalk("heavy_walk", {
    fps: 20,
    duration: 1.48,
    cycleDistance: 0.62,
    gait: "walk",
    loop: true,
    samples: 25,
    contactParts: {
      frontLeft: "foot_front_l",
      frontRight: "foot_front_r",
      backLeft: "foot_rear_l",
      backRight: "foot_rear_r",
    },
    body: "body",
    bodyBob: 0.008,
    bodyBobCenter: 0.01,
    head: "neck_1",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "upper_leg_front_l",
      frontRight: "upper_leg_front_r",
      backLeft: "upper_leg_rear_l",
      backRight: "upper_leg_rear_r",
    },
    stanceRatio: 0.74,
    swingDegrees: 11,
    tail: "tail_1",
    tailSwingDegrees: 6,
    tracks: [
      followThrough("neck_2", { source: "neck_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 4, overshoot: 0.35, lag: 0.08 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 9, overshoot: 0.4, lag: 0.08 }),
      followThrough("tail_3", { source: "tail_2", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 12, overshoot: 0.45, lag: 0.09 }),
      followThrough("tail_4", { source: "tail_3", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 15, overshoot: 0.5, lag: 0.1 }),
      swing("tongue_root", { axis: "x", degrees: 5, frequency: 2, phase: 0.1 }),
      swing("tongue_l", { axis: "y", degrees: 5, frequency: 2, phase: 0.2 }),
      swing("tongue_r", { axis: "y", degrees: -5, frequency: 2, phase: 0.2 }),
    ],
  });
});
