import { figure } from "../../src/dsl";

// A box-only male ostrich with a compact black torso, tiny white-edged wings,
// long two-stage bare neck, small head, and powerful two-stage running legs.
export default figure("ostrich", ({
  mat,
  asciiTexture,
  part,
  box,
  bipedWalk,
  swing,
  followThrough,
}) => {
  mat("black", "#242422");
  mat("black_light", "#3a3b37");
  mat("white", "#eee9dc");
  mat("white_shadow", "#cfc9bd");
  mat("skin", "#b79585");
  mat("skin_light", "#d0ae9b");
  mat("beak", "#d8b18b");
  mat("eye", "#181513");
  mat("leg", "#a78474");
  mat("foot", "#5c4c44");

  asciiTexture("face", {
    palette: { ".": "#d0ae9b", "e": "#181513", "l": "#e2c5b2", "d": "#8d6e63" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      "..e..e..",
      "...ll...",
      "..llll..",
      "........",
    ],
  });
  asciiTexture("body_plumes", {
    palette: { ".": "#242422", "l": "#3a3b37", "w": "#eee9dc" },
    pixels: [
      "llllllllllll",
      "l..........l",
      "..ll..ll....",
      ".l..ll..l...",
      "............",
      "ww........ww",
      "wwww....wwww",
    ],
  });
  asciiTexture("wing_plumes", {
    palette: { ".": "#242422", "l": "#3a3b37", "w": "#eee9dc", "s": "#cfc9bd" },
    pixels: [
      "llllllllll",
      "l........l",
      "..ll..ll..",
      "..........",
      "wwwwwwwwww",
      "wssssssssw",
    ],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#5c4c44", "d": "#302824" },
    pixels: ["........", "....dd..", "..dddddd", "dddddddd"],
  });

  part("body", box({
    at: [0, 1.62, 0.1],
    size: [0.88, 0.86, 1.08],
    material: "black",
    faces: {
      east: { texture: "body_plumes" },
      west: { texture: "body_plumes" },
    },
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.04, -0.6],
    size: [0.68, 0.68, 0.22],
    material: "black_light",
  }));
  for (const [side, x, face] of [
    ["l", -0.49, "west"],
    ["r", 0.49, "east"],
  ] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [x, 0.03, 0.02],
      size: [0.1, 0.58, 0.78],
      material: "black_light",
      faces: { [face]: { texture: "wing_plumes" } },
      joint: { pivot: [side === "l" ? 0.04 : -0.04, 0.2, -0.22], axis: [0, 0, 1] },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [0, -0.28, 0.04],
      size: [0.11, 0.22, 0.52],
      material: "white",
    }));
  }
  part("neck_lower", box({
    parent: "body",
    at: [0, 0.75, -0.38],
    rot: [-9, 0, 0],
    size: [0.28, 1.02, 0.28],
    material: "skin",
    joint: { pivot: [0, -0.49, 0.07], axis: [1, 0, 0] },
  }));
  part("neck_upper", box({
    parent: "neck_lower",
    at: [0, 0.65, -0.08],
    rot: [7, 0, 0],
    size: [0.24, 0.62, 0.24],
    material: "skin_light",
  }));
  part("head", box({
    parent: "neck_upper",
    at: [0, 0.4, -0.12],
    rot: [5, 0, 0],
    size: [0.4, 0.38, 0.46],
    material: "skin_light",
    faces: { north: { texture: "face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.06, -0.36],
    size: [0.34, 0.17, 0.28],
    material: "beak",
  }));
  part("beak_tip", box({
    parent: "beak",
    at: [0, -0.02, -0.18],
    size: [0.26, 0.13, 0.12],
    material: "foot",
  }));
  part("tail_root", box({
    parent: "body",
    at: [0, 0.14, 0.62],
    size: [0.62, 0.38, 0.2],
    material: "white_shadow",
    joint: { pivot: [0, -0.16, -0.08], axis: [1, 0, 0] },
  }));
  for (const [name, x, lean] of [
    ["tail_l", -0.25, -11],
    ["tail_c", 0, 0],
    ["tail_r", 0.25, 11],
  ] as const) {
    part(name, box({
      parent: "tail_root",
      at: [x, 0.22, 0.13],
      rot: [-5, 0, lean],
      size: [0.25, name === "tail_c" ? 0.58 : 0.48, 0.11],
      material: "white",
    }));
  }

  for (const [side, x] of [["l", -0.28], ["r", 0.28]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.72, 0.02],
      size: [0.18, 0.72, 0.2],
      material: "leg",
      joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.58, 0.05],
      rot: [3, 0, 0],
      size: [0.14, 0.58, 0.16],
      material: "skin",
    }));
    part(`foot_${side}`, box({
      parent: `shin_${side}`,
      at: [0, -0.34, -0.2],
      size: [0.28, 0.12, 0.68],
      material: "foot",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  bipedWalk("run", {
    fps: 20,
    duration: 0.74,
    cycleDistance: 1.28,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.028,
    bodyBobCenter: 0.03,
    head: "head",
    headSwingDegrees: 3,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.55,
    swingDegrees: 25,
    tracks: [
      swing("wing_l", { axis: "z", degrees: 5, center: -2, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -5, center: 2, frequency: 2 }),
      followThrough("neck_lower", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.55, lag: 0.11 }),
      followThrough("tail_root", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.6, lag: 0.14 }),
    ],
  });
});
