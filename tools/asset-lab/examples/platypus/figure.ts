import { figure } from "../../src/dsl";

// A box-only duck-billed platypus with a low waterproof body, broad slate
// bill, four webbed feet, and a wide two-stage paddle tail. Its swim cycle
// alternates all four feet while a gentle tail wave steadies the body.
export default figure("platypus", ({
  mat,
  asciiTexture,
  part,
  box,
  swim,
  swing,
}) => {
  mat("fur", "#604638");
  mat("fur_light", "#7d5d49");
  mat("fur_dark", "#3b302a");
  mat("belly", "#a88c70");
  mat("bill", "#4c5152");
  mat("bill_light", "#676c6b");
  mat("eye", "#151413");
  mat("web", "#413832");
  mat("tail", "#4a382e");

  asciiTexture("face", {
    palette: { ".": "#7d5d49", "d": "#604638", "e": "#151413", "l": "#a88c70" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      ".ee..ee.",
      "..llll..",
      "........",
      "........",
    ],
  });
  asciiTexture("bill_top", {
    palette: { ".": "#4c5152", "l": "#676c6b", "n": "#25292a" },
    pixels: [
      "llllllllll",
      "l........l",
      "..nn..nn..",
      "..........",
      ".llllllll.",
      "llllllllll",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#604638", "l": "#7d5d49", "d": "#3b302a", "b": "#a88c70" },
    pixels: [
      "dddddddddddd",
      "dlllllllllld",
      "l..........l",
      "............",
      "..bbbbbbbb..",
      ".bbbbbbbbbb.",
    ],
  });
  asciiTexture("web_top", {
    palette: { ".": "#413832", "l": "#66564a" },
    pixels: [
      "...ll...",
      ".ll..ll.",
      "llllllll",
      "llllllll",
    ],
  });
  asciiTexture("tail_top", {
    palette: { ".": "#4a382e", "l": "#705748", "d": "#30261f" },
    pixels: [
      "dddddddddd",
      "dlllllllll",
      "dl.......l",
      "d.l......l",
      "d..l.....l",
      "d...l....l",
      "d....lllll",
      "dddddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.56, 0.06],
    size: [0.84, 0.44, 1.22],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.27, -0.02],
    size: [0.64, 0.11, 0.86],
    material: "belly",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.04, -0.79],
    size: [0.6, 0.46, 0.5],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.21], axis: [0, 1, 0] },
  }));
  part("bill", box({
    parent: "head",
    at: [0, -0.1, -0.43],
    size: [0.72, 0.16, 0.5],
    material: "bill",
    faces: { up: { texture: "bill_top" } },
  }));
  part("lower_bill", box({
    parent: "bill",
    at: [0, -0.105, 0.02],
    size: [0.62, 0.07, 0.43],
    material: "bill_light",
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.32, -0.36],
    ["fr", 0.32, -0.36],
    ["bl", -0.32, 0.38],
    ["br", 0.32, 0.38],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.3, z],
      size: [0.14, 0.3, 0.16],
      material: "fur_dark",
      joint: { pivot: [0, 0.15, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.19, -0.08],
      size: [0.36, 0.1, 0.42],
      material: "web",
      faces: { up: { texture: "web_top" } },
    }));
  }

  part("tail_base", box({
    parent: "body",
    at: [0, -0.02, 0.77],
    size: [0.48, 0.18, 0.48],
    material: "tail",
    joint: { pivot: [0, 0, -0.22], axis: [0, 1, 0] },
  }));
  part("tail_paddle", box({
    parent: "tail_base",
    at: [0, -0.02, 0.5],
    size: [0.7, 0.13, 0.66],
    material: "tail",
    faces: { up: { texture: "tail_top" } },
    joint: { pivot: [0, 0, -0.3], axis: [0, 1, 0] },
  }));

  swim("paddle", {
    fps: 18,
    duration: 1.16,
    cycleDistance: 0.82,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.012,
    bodySwayDegrees: 1.8,
    tail: "tail_base",
    tailAxis: "y",
    tailSwingDegrees: 7,
    tailTip: "tail_paddle",
    tailTipPhase: 0.12,
    tailTipSwingDegrees: 11,
    tracks: [
      swing("leg_fl", { axis: "x", degrees: 24, phase: 0 }),
      swing("leg_fr", { axis: "x", degrees: 24, phase: 0.5 }),
      swing("leg_bl", { axis: "x", degrees: 18, phase: 0.5 }),
      swing("leg_br", { axis: "x", degrees: 18, phase: 0 }),
      swing("head", { axis: "y", degrees: 2, phase: 0.5 }),
    ],
  });
});
