import { figure } from "../../src/dsl";

// A box-only harbor seal with a tapered spotted body, round earless head,
// pale whiskered muzzle, short foreflippers, and paired rear flippers carried
// by a small vertically flexing pelvis segment.
export default figure("harbor_seal", ({
  mat,
  asciiTexture,
  part,
  box,
  swim,
  swing,
}) => {
  mat("fur", "#89877d");
  mat("fur_light", "#b8b3a5");
  mat("fur_dark", "#4c4c48");
  mat("belly", "#d0cab9");
  mat("muzzle", "#c8bca7");
  mat("nose", "#1d1d1c");

  asciiTexture("face", {
    palette: { ".": "#89877d", "l": "#b8b3a5", "e": "#1d1d1c", "s": "#4c4c48" },
    pixels: [
      "ss......ss",
      "s.e....e.s",
      "..e....e..",
      "...llll...",
      "..llllll..",
      "..........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#c8bca7", "n": "#1d1d1c", "w": "#eee8d9" },
    pixels: [
      "w........w",
      ".w..nn..w.",
      "...nnnn...",
      ".w..nn..w.",
      "w........w",
    ],
  });
  asciiTexture("spotted_side", {
    palette: { ".": "#89877d", "l": "#b8b3a5", "s": "#4c4c48", "b": "#d0cab9" },
    pixels: [
      "ss...........ss",
      "s..ll..s..ll..s",
      "..s...ll...s...",
      "....s....s.....",
      "..ll..s.....ll.",
      ".bbbbbbbbbbbbb.",
      "bbbbbbbbbbbbbbb",
    ],
  });
  asciiTexture("flipper_spots", {
    palette: { ".": "#89877d", "s": "#4c4c48", "l": "#b8b3a5" },
    pixels: ["ss......", "..ss..l.", "....ss..", ".l....ss", "..l....."],
  });

  part("body", box({
    at: [0, 0.72, 0.12],
    size: [0.82, 0.58, 1.58],
    material: "fur",
    faces: {
      east: { texture: "spotted_side" },
      west: { texture: "spotted_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.34, -0.04],
    size: [0.66, 0.12, 1.18],
    material: "belly",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.08, -0.66],
    size: [0.88, 0.6, 0.46],
    material: "fur_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.06, 0.66],
    size: [0.64, 0.46, 0.52],
    material: "fur_dark",
  }));
  part("head", box({
    parent: "shoulders",
    at: [0, 0.03, -0.43],
    size: [0.7, 0.58, 0.52],
    material: "fur_light",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.15, -0.38],
    size: [0.54, 0.3, 0.28],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("nose", box({
    parent: "muzzle",
    at: [0, 0.04, -0.18],
    size: [0.2, 0.14, 0.1],
    material: "nose",
  }));
  for (const [side, x, yaw, roll] of [
    ["l", -0.49, 9, -8],
    ["r", 0.49, -9, 8],
  ] as const) {
    part(`foreflipper_${side}`, box({
      parent: "body",
      at: [x, -0.22, -0.4],
      rot: [0, yaw, roll],
      size: [0.48, 0.1, 0.34],
      material: "fur_dark",
      faces: {
        up: { texture: "flipper_spots" },
        down: { texture: "flipper_spots" },
      },
      joint: { pivot: [side === "l" ? 0.22 : -0.22, 0, -0.06], axis: [0, 0, 1] },
    }));
  }
  part("pelvis", box({
    parent: "rump",
    at: [0, -0.02, 0.43],
    size: [0.34, 0.28, 0.48],
    material: "fur_dark",
    joint: { pivot: [0, 0, -0.22], axis: [1, 0, 0] },
  }));
  part("hind_flipper_l", box({
    parent: "pelvis",
    at: [-0.25, 0, 0.38],
    rot: [0, 15, -5],
    size: [0.48, 0.09, 0.54],
    material: "fur_dark",
    faces: { up: { texture: "flipper_spots" } },
    joint: { pivot: [0.18, 0, -0.22], axis: [0, 0, 1] },
  }));
  part("hind_flipper_r", box({
    parent: "pelvis",
    at: [0.25, 0, 0.38],
    rot: [0, -15, 5],
    size: [0.48, 0.09, 0.54],
    material: "fur_dark",
    faces: { up: { texture: "flipper_spots" } },
    joint: { pivot: [-0.18, 0, -0.22], axis: [0, 0, 1] },
  }));

  swim("swim", {
    fps: 18,
    duration: 1.14,
    cycleDistance: 1.24,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.025,
    bodySwayDegrees: 2,
    finAxis: "z",
    finPhase: 0.18,
    finSwingDegrees: 12,
    leftFin: "foreflipper_l",
    rightFin: "foreflipper_r",
    tail: "pelvis",
    tailAxis: "x",
    tailSwingDegrees: 12,
    tracks: [
      swing("hind_flipper_l", { axis: "z", degrees: 7, phase: 0.15 }),
      swing("hind_flipper_r", { axis: "z", degrees: -7, phase: 0.15 }),
    ],
  });
});
