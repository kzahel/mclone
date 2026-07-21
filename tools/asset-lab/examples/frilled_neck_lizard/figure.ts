import { figure, type ClipKey } from "../../src/dsl";

// A box-only frilled-neck lizard with sandy scales, broad head, four sprawled
// legs, long banded tail, and a separate expanding collar display.
export default figure("frilled_neck_lizard", ({
  asciiTexture,
  bob,
  box,
  clip,
  defaultClip,
  mat,
  part,
  swing,
  walkCycle,
}) => {
  mat("sand", "#a87948");
  mat("sand_light", "#c79b61");
  mat("sand_dark", "#684b36");
  mat("cream", "#dfc38e");
  mat("frill", "#d56f42");
  mat("frill_light", "#e6a353");
  mat("frill_dark", "#8d3e32");
  mat("eye", "#dbba46");
  mat("pupil", "#171712");
  mat("toe", "#514238");

  asciiTexture("scale_bands", {
    palette: { ".": "#a87948", "l": "#c79b61", "d": "#684b36", "c": "#dfc38e" },
    pixels: [
      "llllllllllll",
      "l..........l",
      "..dd..dd....",
      ".d..dd..d...",
      "....cccc....",
      "dd........dd",
      "llllllllllll",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#c79b61", "d": "#684b36", "e": "#dbba46", "p": "#171712" },
    pixels: ["dd......dd", "d.ep..pe.d", "..ep..pe..", "..........", ".dddddddd.", ".........."],
  });
  asciiTexture("frill_pattern", {
    palette: { ".": "#d56f42", "l": "#e6a353", "d": "#8d3e32", "c": "#dfc38e" },
    pixels: ["dddddddd", "dll..lld", "dl.cc.ld", "d.c..c.d", "dll..lld", "dddddddd"],
  });
  asciiTexture("toes", {
    palette: { ".": "#514238", "l": "#dfc38e" },
    pixels: ["ll..ll", ".llll.", "......"],
  });

  part("body", box({
    at: [0, 0.4, 0.05],
    size: [0.6, 0.3, 1.08],
    material: "sand",
    faces: { up: { texture: "scale_bands" }, east: { texture: "scale_bands" }, west: { texture: "scale_bands" } },
  }));
  part("back_ridge", box({
    parent: "body",
    at: [0, 0.2, 0.04],
    size: [0.34, 0.12, 0.76],
    material: "sand_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.19, -0.04],
    size: [0.48, 0.1, 0.78],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.02, -0.63],
    size: [0.46, 0.28, 0.34],
    material: "sand_dark",
    joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.07, -0.34],
    size: [0.66, 0.34, 0.46],
    material: "sand_light",
    faces: { north: { texture: "face" } },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.04, -0.31],
    size: [0.54, 0.22, 0.22],
    material: "sand_light",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.27, 0.15, -0.08],
      size: [0.18, 0.16, 0.18],
      material: "sand_dark",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0, -0.11],
      size: [0.1, 0.09, 0.055],
      material: "eye",
      faces: { north: { material: "pupil" } },
    }));
  }

  const frillParts = [
    ["frill_upper_l", -1, 1, -0.07],
    ["frill_upper_r", 1, 1, -0.085],
    ["frill_lower_l", -1, -1, -0.1],
    ["frill_lower_r", 1, -1, -0.115],
  ] as const;
  for (const [name, sideSign, verticalSign, z] of frillParts) {
    part(name, box({
      parent: "neck",
      at: [sideSign * 0.26, verticalSign * 0.11, z],
      rot: [0, sideSign * 5, sideSign * verticalSign * 10],
      size: [0.3, 0.29, 0.065],
      material: verticalSign > 0 ? "frill_light" : "frill",
      faces: { north: { texture: "frill_pattern" }, south: { texture: "frill_pattern" } },
      joint: { pivot: [sideSign * -0.13, verticalSign * -0.11, 0], axis: [0, 0, 1] },
    }));
  }

  for (const [row, z, bend] of [["front", -0.35, -0.09], ["rear", 0.34, 0.09]] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "body",
        at: [sign * 0.42, -0.12, z],
        rot: [0, sign * (row === "front" ? -12 : 12), sign * 8],
        size: [0.42, 0.11, 0.15],
        material: "sand_dark",
        joint: { pivot: [sign * -0.19, 0, 0], axis: [0, 1, 0] },
      }));
      part(`forearm_${row}_${side}`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.3, -0.1, bend],
        rot: [0, 0, sign * 17],
        size: [0.36, 0.09, 0.13],
        material: "sand_light",
        joint: { pivot: [sign * -0.16, 0, 0], axis: [0, 1, 0] },
      }));
      part(`foot_${row}_${side}`, box({
        parent: `forearm_${row}_${side}`,
        at: [sign * 0.23, -0.07, bend * 0.8],
        size: [0.27, 0.065, 0.3],
        material: "toe",
        faces: { up: { texture: "toes" } },
      }));
    }
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.01, 0.73],
    size: [0.42, 0.24, 0.62],
    material: "sand",
    faces: { up: { texture: "scale_bands" }, east: { texture: "scale_bands" }, west: { texture: "scale_bands" } },
    joint: { pivot: [0, 0, -0.28], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.01, 0.51],
    size: [0.3, 0.18, 0.54],
    material: "sand_light",
    joint: { pivot: [0, 0, -0.25], axis: [0, 1, 0] },
  }));
  part("tail_3", box({
    parent: "tail_2",
    at: [0, -0.01, 0.45],
    size: [0.2, 0.14, 0.48],
    material: "sand_dark",
    joint: { pivot: [0, 0, -0.22], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_3",
    at: [0, -0.01, 0.4],
    size: [0.12, 0.1, 0.42],
    material: "cream",
    joint: { pivot: [0, 0, -0.19], axis: [0, 1, 0] },
  }));

  walkCycle("sand_scuttle", {
    label: "Sand scuttle",
    role: "locomotion",
    fps: 24,
    duration: 0.74,
    loop: true,
    samples: 25,
    locomotion: { kind: "quadruped-walk", cycleDistance: 0.68, direction: [0, 0, -1], units: "figure" },
    tracks: [
      bob("body", { axis: "y", amount: 0.008, center: 0.008, phase: 0.5 }),
      swing("body", { axis: "y", degrees: 1.7 }),
      swing("neck", { axis: "y", degrees: 3, phase: 0.5 }),
      swing("leg_front_l", { axis: "y", degrees: 18, phase: 0 }),
      swing("leg_rear_r", { axis: "y", degrees: -18, phase: 0 }),
      swing("leg_front_r", { axis: "y", degrees: -18, phase: 0.5 }),
      swing("leg_rear_l", { axis: "y", degrees: 18, phase: 0.5 }),
      swing("forearm_front_l", { axis: "y", degrees: -11, phase: 0 }),
      swing("forearm_rear_r", { axis: "y", degrees: 11, phase: 0 }),
      swing("forearm_front_r", { axis: "y", degrees: 11, phase: 0.5 }),
      swing("forearm_rear_l", { axis: "y", degrees: -11, phase: 0.5 }),
      swing("tail_1", { axis: "y", degrees: 7, phase: 0.08 }),
      swing("tail_2", { axis: "y", degrees: 11, phase: 0.18 }),
      swing("tail_3", { axis: "y", degrees: 14, phase: 0.28 }),
      swing("tail_tip", { axis: "y", degrees: 17, phase: 0.38 }),
    ],
  });
  const displayKeys: ClipKey[] = [
    ["neck", 0, { rot: [0, 0, 0] }],
    ["neck", 0.18, { rot: [-4, 0, 0] }],
    ["neck", 0.42, { rot: [-4, 0, 0] }],
    ["neck", 0.72, { rot: [0, 0, 0] }],
  ];
  for (const [name, sideSign, verticalSign] of frillParts) {
    displayKeys.push(
      [name, 0, { at: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] }],
      [name, 0.18, {
        at: [sideSign * 0.13, verticalSign > 0 ? 0.07 : 0.12, -0.03],
        rot: [0, sideSign * -8, sideSign * verticalSign * (verticalSign > 0 ? 18 : 13)],
        scale: [1.65, verticalSign > 0 ? 1.55 : 1.32, 1],
      }],
      [name, 0.42, {
        at: [sideSign * 0.14, verticalSign > 0 ? 0.075 : 0.13, -0.03],
        rot: [0, sideSign * -9, sideSign * verticalSign * (verticalSign > 0 ? 20 : 14)],
        scale: [1.72, verticalSign > 0 ? 1.62 : 1.36, 1],
      }],
      [name, 0.72, { at: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] }],
    );
  }
  clip("frill_display", {
    label: "Frill display",
    role: "action",
    nextClip: "sand_scuttle",
    fps: 24,
    loop: false,
    keys: displayKeys,
  });
  defaultClip("sand_scuttle");
});
