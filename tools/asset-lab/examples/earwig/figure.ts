import { figure } from "../../src/dsl";

// A box-only common earwig with an elongated five-band abdomen, long two-stage
// antennae, six two-stage legs, and paired three-part rear forceps.
export default figure("earwig", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  contactSwing,
  bob,
  swing,
}) => {
  mat("brown", "#6f4429");
  mat("brown_light", "#9b6337");
  mat("brown_dark", "#3d2d24");
  mat("chestnut", "#7f3f28");
  mat("armor", "#573326");
  mat("leg", "#5d4030");
  mat("leg_tip", "#29221d");
  mat("eye", "#151311");

  asciiTexture("face", {
    palette: { ".": "#573326", "e": "#151311", "h": "#c19b68", "l": "#9b6337" },
    pixels: [
      "........",
      ".ee..ee.",
      ".eh..he.",
      "..llll..",
      ".llllll.",
    ],
  });
  asciiTexture("abdomen_band", {
    palette: { ".": "#6f4429", "l": "#9b6337", "d": "#3d2d24", "c": "#7f3f28" },
    pixels: [
      "llllllllll",
      "l........l",
      ".d.cc.cc.d",
      "d.cc..cc.d",
      ".d.cc.cc.d",
      "dddddddddd",
    ],
  });
  asciiTexture("forceps_ridge", {
    palette: { ".": "#573326", "l": "#9b6337", "d": "#29221d" },
    pixels: [
      "llllllll",
      "l......l",
      ".d.dd.d.",
      "dddddddd",
    ],
  });

  part("abdomen", box({
    at: [0, 0.48, 0.16],
    size: [0.5, 0.28, 1.08],
    material: "brown_dark",
  }));
  for (const [index, z, width, height, material] of [
    [1, -0.38, 0.46, 0.18, "brown_light"],
    [2, -0.19, 0.48, 0.2, "brown"],
    [3, 0, 0.5, 0.22, "chestnut"],
    [4, 0.2, 0.47, 0.19, "brown"],
    [5, 0.39, 0.43, 0.17, "armor"],
  ] as const) {
    part(`abdomen_band_${index}`, box({
      parent: "abdomen",
      at: [0, 0.18 + height * 0.08, z],
      size: [width, height, 0.17],
      material,
      faces: {
        up: { texture: "abdomen_band" },
        east: { texture: "abdomen_band" },
        west: { texture: "abdomen_band" },
      },
    }));
  }
  part("thorax", box({
    parent: "abdomen",
    at: [0, 0.035, -0.68],
    size: [0.54, 0.3, 0.34],
    material: "chestnut",
    faces: { up: { texture: "abdomen_band" } },
  }));
  part("head", box({
    parent: "thorax",
    at: [0, -0.02, -0.31],
    size: [0.48, 0.25, 0.28],
    material: "armor",
    faces: { north: { texture: "face" } },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`antenna_${side}`, box({
      parent: "head",
      at: [sign * 0.16, 0.16, -0.08],
      rot: [-26, 0, sign * -24],
      size: [0.035, 0.38, 0.035],
      material: "leg",
      joint: { pivot: [0, -0.18, 0], axis: [0, 0, 1] },
    }));
    part(`antenna_${side}_tip`, box({
      parent: `antenna_${side}`,
      at: [0, 0.29, -0.03],
      rot: [-17, 0, sign * -7],
      size: [0.028, 0.3, 0.028],
      material: "leg_tip",
    }));
  }

  for (const [row, z, yaw] of [["front", -0.3, 22], ["mid", -0.02, 0], ["rear", 0.28, -22]] as const) {
    for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "abdomen",
        at: [sign * 0.32, -0.14, z],
        rot: [0, sign * yaw, sign * -21],
        size: [0.34, 0.05, 0.06],
        material: "leg",
        joint: { pivot: [sign * -0.16, 0, 0], axis: [0, 1, 0] },
      }));
      part(`leg_${row}_${side}_tip`, box({
        parent: `leg_${row}_${side}`,
        at: [sign * 0.29, -0.055, 0],
        rot: [0, 0, sign * -18],
        size: [0.27, 0.045, 0.05],
        material: "leg_tip",
      }));
    }
  }

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`forceps_${side}_base`, box({
      parent: "abdomen",
      at: [sign * 0.14, 0.01, 0.7],
      rot: [2, sign * 18, 0],
      size: [0.095, 0.09, 0.4],
      material: "armor",
      faces: { up: { texture: "forceps_ridge" } },
      joint: { pivot: [0, 0, -0.18], axis: [0, 1, 0] },
    }));
    part(`forceps_${side}_mid`, box({
      parent: `forceps_${side}_base`,
      at: [0, 0, 0.36],
      rot: [0, sign * -30, 0],
      size: [0.08, 0.08, 0.34],
      material: "brown_light",
      joint: { pivot: [0, 0, -0.15], axis: [0, 1, 0] },
    }));
    part(`forceps_${side}_tip`, box({
      parent: `forceps_${side}_mid`,
      at: [0, 0, 0.29],
      rot: [0, sign * -24, 0],
      size: [0.065, 0.065, 0.23],
      material: "leg_tip",
      joint: { pivot: [0, 0, -0.1], axis: [0, 1, 0] },
    }));
  }

  walkCycle("scuttle", {
    fps: 20,
    duration: 0.94,
    loop: true,
    samples: 21,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.64,
      direction: [0, 0, -1],
      units: "figure",
      contacts: [
        { part: "leg_front_l_tip", phaseStart: 0, phaseEnd: 0.64, role: "front-left", stanceRatio: 0.64 },
        { part: "leg_mid_r_tip", phaseStart: 0, phaseEnd: 0.64, role: "middle-right", stanceRatio: 0.64 },
        { part: "leg_rear_l_tip", phaseStart: 0, phaseEnd: 0.64, role: "rear-left", stanceRatio: 0.64 },
        { part: "leg_front_r_tip", phaseStart: 0.5, phaseEnd: 0.14, role: "front-right", stanceRatio: 0.64 },
        { part: "leg_mid_l_tip", phaseStart: 0.5, phaseEnd: 0.14, role: "middle-left", stanceRatio: 0.64 },
        { part: "leg_rear_r_tip", phaseStart: 0.5, phaseEnd: 0.14, role: "rear-right", stanceRatio: 0.64 },
      ],
    },
    tracks: [
      bob("abdomen", { axis: "y", amount: 0.012, center: 0.012, phase: 0.5 }),
      contactSwing("leg_front_l", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.64 }),
      contactSwing("leg_mid_r", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.64 }),
      contactSwing("leg_rear_l", { axis: "y", degrees: 15, phase: 0, stanceRatio: 0.64 }),
      contactSwing("leg_front_r", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.64 }),
      contactSwing("leg_mid_l", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.64 }),
      contactSwing("leg_rear_r", { axis: "y", degrees: 15, phase: 0.5, stanceRatio: 0.64 }),
      swing("forceps_l_base", { axis: "y", degrees: 7, center: 2, phase: 0.5, frequency: 0.5 }),
      swing("forceps_r_base", { axis: "y", degrees: -7, center: -2, phase: 0.5, frequency: 0.5 }),
      swing("antenna_l", { axis: "z", degrees: 5, phase: 0.25 }),
      swing("antenna_r", { axis: "z", degrees: -5, phase: 0.25 }),
    ],
  });
});
