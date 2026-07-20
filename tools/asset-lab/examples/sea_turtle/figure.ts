import { figure } from "../../src/dsl";

// A box-only adult green sea turtle with a broad stepped shell, blunt beaked
// head, long front paddles, short rear paddles, and a tiny tail. The shared
// swim cycle drives a slow mirrored power stroke rather than tail propulsion.
export default figure("sea_turtle", ({
  mat,
  asciiTexture,
  part,
  box,
  swim,
  swing,
}) => {
  mat("shell", "#476745");
  mat("shell_light", "#718555");
  mat("shell_dark", "#2d4935");
  mat("scute", "#89905b");
  mat("skin", "#6d8c67");
  mat("skin_light", "#93a579");
  mat("belly", "#c5bc83");
  mat("eye", "#17170f");
  mat("beak", "#d1c595");

  asciiTexture("shell_top", {
    palette: { "d": "#2d4935", "s": "#476745", "l": "#718555", "c": "#89905b" },
    pixels: [
      "dddddddddddd",
      "dccccccccccd",
      "dcclllllcccd",
      "dclcssssclcd",
      "dclcssssclcd",
      "dcclllllcccd",
      "dccccccccccd",
      "dddddddddddd",
    ],
  });
  asciiTexture("shell_side", {
    palette: { ".": "#476745", "d": "#2d4935", "l": "#718555", "b": "#c5bc83" },
    pixels: [
      "dddddddddddd",
      "dll..ll..lld",
      "l..........l",
      "............",
      ".bbbbbbbbbb.",
      "bbbbbbbbbbbb",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#6d8c67", "l": "#93a579", "e": "#17170f" },
    pixels: [
      "ll....ll",
      ".ee..ee.",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("flipper_spots", {
    palette: { ".": "#6d8c67", "d": "#476745", "l": "#93a579" },
    pixels: [
      "dd......",
      "..dd..l.",
      "....dd..",
      ".l....dd",
      "..l.....",
    ],
  });

  part("shell", box({
    at: [0, 0.72, 0.08],
    size: [1.16, 0.46, 1.46],
    material: "shell",
    faces: {
      up: { texture: "shell_top" },
      east: { texture: "shell_side" },
      west: { texture: "shell_side" },
    },
  }));
  part("shell_ridge", box({
    parent: "shell",
    at: [0, 0.29, 0.04],
    size: [0.82, 0.18, 1.08],
    material: "shell_light",
    faces: { up: { texture: "shell_top" } },
  }));
  part("plastron", box({
    parent: "shell",
    at: [0, -0.3, 0.02],
    size: [0.92, 0.14, 1.16],
    material: "belly",
  }));

  part("neck", box({
    parent: "shell",
    at: [0, -0.05, -0.83],
    size: [0.4, 0.34, 0.36],
    material: "skin_light",
    joint: { pivot: [0, 0, 0.16], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.02, -0.34],
    size: [0.52, 0.42, 0.46],
    material: "skin",
    faces: { north: { texture: "face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.1, -0.3],
    size: [0.32, 0.18, 0.18],
    material: "beak",
  }));

  part("flipper_l", box({
    parent: "shell",
    at: [-0.78, -0.08, -0.34],
    rot: [0, 14, -7],
    size: [0.82, 0.1, 0.46],
    material: "skin",
    faces: { up: { texture: "flipper_spots" } },
    joint: { pivot: [0.38, 0, -0.08], axis: [0, 0, 1] },
  }));
  part("flipper_r", box({
    parent: "shell",
    at: [0.78, -0.08, -0.34],
    rot: [0, -14, 7],
    size: [0.82, 0.1, 0.46],
    material: "skin",
    faces: { up: { texture: "flipper_spots" } },
    joint: { pivot: [-0.38, 0, -0.08], axis: [0, 0, 1] },
  }));
  part("rear_flipper_l", box({
    parent: "shell",
    at: [-0.65, -0.13, 0.54],
    rot: [0, -10, -4],
    size: [0.5, 0.09, 0.38],
    material: "skin_light",
    joint: { pivot: [0.23, 0, 0.04], axis: [0, 0, 1] },
  }));
  part("rear_flipper_r", box({
    parent: "shell",
    at: [0.65, -0.13, 0.54],
    rot: [0, 10, 4],
    size: [0.5, 0.09, 0.38],
    material: "skin_light",
    joint: { pivot: [-0.23, 0, 0.04], axis: [0, 0, 1] },
  }));
  part("tail", box({
    parent: "shell",
    at: [0, -0.12, 0.85],
    size: [0.16, 0.18, 0.3],
    material: "skin",
    joint: { pivot: [0, 0, -0.14], axis: [0, 1, 0] },
  }));

  swim("swim", {
    fps: 18,
    duration: 1.5,
    cycleDistance: 0.92,
    loop: true,
    samples: 21,
    body: "shell",
    bodyBob: 0.018,
    bodySwayDegrees: 1.2,
    finAxis: "z",
    finPhase: 0.15,
    finSwingDegrees: 24,
    leftFin: "flipper_l",
    rightFin: "flipper_r",
    tail: "tail",
    tailSwingDegrees: 4,
    tracks: [
      swing("rear_flipper_l", { axis: "z", degrees: 8, phase: 0.32 }),
      swing("rear_flipper_r", { axis: "z", degrees: -8, phase: 0.32 }),
      swing("neck", { axis: "y", degrees: 2, phase: 0.5 }),
    ],
  });
});
