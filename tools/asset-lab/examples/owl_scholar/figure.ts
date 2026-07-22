import { figure } from "../../src/dsl";

// A recognizable spectacled owl with a satchel and small open card-page book,
// retaining an avian silhouette while adding a perch-hop and page-turn action.
export default figure("owl_scholar", ({
  asciiTexture,
  bipedWalk,
  box,
  clip,
  defaultClip,
  mat,
  metadata,
  part,
  plane,
}) => {
  metadata({
    bodyPlans: ["biped", "winged"],
    disposition: "neutral",
    groups: ["animal", "fantasy"],
    habitats: ["land", "air"],
    scale: "small",
    themes: ["book", "owl", "scholar", "woodland"],
  });

  mat("feather", "#6b5545");
  mat("feather_light", "#b89b73");
  mat("feather_dark", "#302923");
  mat("disc", "#ddc99c");
  mat("beak", "#d69c36");
  mat("talon", "#a87832");
  mat("gold", "#c8a64c");
  mat("spectacle", "#31566a");
  mat("leather", "#70402f");
  mat("leather_dark", "#3c2a24");
  mat("page", { color: "#ded2aa", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("ink", "#4d4335");

  asciiTexture("scholar_face", {
    palette: { ".": "#ddc99c", "r": "#6b5545", "d": "#302923", "e": "#e4b84b" },
    pixels: [
      "rr......rr",
      "r..dddd..r",
      "..de..ed..",
      "..dd..dd..",
      "....dd....",
      "...rrrr...",
      "..r....r..",
      "..........",
    ],
  });
  asciiTexture("scholar_breast", {
    palette: { ".": "#b89b73", "d": "#302923", "l": "#ddc99c" },
    pixels: ["l.l..l.l", ".d....d.", "..l..l..", ".d....d.", "l..ll..l", ".d....d.", "..l..l..", "........"],
  });
  asciiTexture("book_page", {
    palette: { ".": "transparent", "p": "#ded2aa", "i": "#4d4335", "r": "#8b4a3d" },
    pixels: [
      ".pppppp.",
      "pppppppp",
      "ppiiiipp",
      "pppppppp",
      "ppiippip",
      "pppppppp",
      "ppirripp",
      ".pppppp.",
    ],
  });

  part("body", box({
    at: [0, 0.73, 0.04],
    size: [0.7, 0.74, 0.66],
    material: "feather",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.04, -0.37],
    size: [0.56, 0.58, 0.14],
    material: "feather_light",
    faces: { north: { texture: "scholar_breast" } },
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.43, -0.16],
    size: [0.8, 0.58, 0.5],
    material: "disc",
    faces: { north: { texture: "scholar_face" } },
    joint: { pivot: [0, -0.22, 0.14], axis: [1, 0, 0] },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.08, -0.32],
    rot: [14, 0, 0],
    size: [0.14, 0.18, 0.16],
    material: "beak",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_tuft_${side}`, box({
      parent: "head",
      at: [sign * 0.28, 0.35, 0.01],
      rot: [0, 0, sign * 14],
      size: [0.12, 0.27, 0.12],
      material: "feather_dark",
    }));
    const lensX = sign * 0.2;
    part(`lens_top_${side}`, box({ parent: "head", at: [lensX, 0.09, -0.28], size: [0.27, 0.04, 0.06], material: "spectacle" }));
    part(`lens_bottom_${side}`, box({ parent: "head", at: [lensX, -0.11, -0.28], size: [0.27, 0.04, 0.06], material: "spectacle" }));
    part(`lens_outer_${side}`, box({ parent: "head", at: [sign * 0.32, -0.01, -0.28], size: [0.04, 0.22, 0.06], material: "spectacle" }));
    part(`lens_inner_${side}`, box({ parent: "head", at: [sign * 0.08, -0.01, -0.28], size: [0.04, 0.22, 0.06], material: "spectacle" }));
    part(`wing_${side}`, box({
      parent: "body",
      at: [sign * 0.44, 0.02, 0],
      rot: [0, sign * -5, sign * 5],
      size: [0.25, 0.62, 0.48],
      material: side === "l" ? "feather" : "feather_dark",
      joint: { pivot: [0, 0.27, 0], axis: [0, 0, 1] },
    }));
    part(`leg_${side}`, box({
      parent: "body",
      at: [sign * 0.18, -0.45, -0.02],
      rot: [-12, 0, 0],
      size: [0.12, 0.3, 0.12],
      material: "talon",
      joint: { pivot: [0, 0.13, 0], axis: [1, 0, 0] },
    }));
    part(`talon_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.22, -0.08],
      size: [0.22, 0.12, 0.3],
      material: "feather_dark",
    }));
  }
  part("spectacle_bridge", box({
    parent: "head",
    at: [0, -0.01, -0.28],
    size: [0.16, 0.04, 0.06],
    material: "spectacle",
  }));

  part("satchel_strap", box({
    parent: "body",
    at: [-0.05, 0.02, -0.36],
    rot: [0, 0, -31],
    size: [0.08, 0.82, 0.08],
    material: "leather_dark",
  }));
  part("satchel", box({
    parent: "body",
    at: [0.36, -0.2, -0.37],
    size: [0.34, 0.3, 0.18],
    material: "leather",
  }));
  part("satchel_clasp", box({
    parent: "satchel",
    at: [0, 0.03, -0.11],
    size: [0.1, 0.1, 0.06],
    material: "gold",
  }));

  part("book_spine", box({
    parent: "body",
    at: [0, -0.08, -0.48],
    size: [0.1, 0.4, 0.1],
    material: "leather_dark",
  }));
  part("page_l", plane({
    parent: "book_spine",
    at: [-0.17, 0, 0],
    rot: [0, 194, 0],
    size: [0.34, 0.38],
    sidedness: "double",
    material: "page",
    texture: "book_page",
    joint: { pivot: [0.16, 0, 0], axis: [0, 1, 0] },
  }));
  part("page_r", plane({
    parent: "book_spine",
    at: [0.17, 0, 0],
    rot: [0, 166, 0],
    size: [0.34, 0.38],
    sidedness: "double",
    material: "page",
    texture: "book_page",
    joint: { pivot: [-0.16, 0, 0], axis: [0, 1, 0] },
  }));

  bipedWalk("perch_hop", {
    label: "Perch hop",
    fps: 22,
    duration: 0.82,
    cycleDistance: 0.38,
    loop: true,
    samples: 21,
    armSwingDegrees: 4,
    body: "body",
    bodyBob: 0.026,
    bodyBobCenter: 0.028,
    head: "head",
    headSwingDegrees: 2.5,
    leftArm: "wing_l",
    leftContact: "talon_l",
    leftLeg: "leg_l",
    rightArm: "wing_r",
    rightContact: "talon_r",
    rightLeg: "leg_r",
    stanceRatio: 0.62,
    swingDegrees: 17,
  });
  clip("page_turn", {
    label: "Page turn",
    role: "action",
    nextClip: "perch_hop",
    fps: 30,
    loop: false,
    keys: [
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.24, { rot: [10, -5, 0] }],
      ["head", 0.58, { rot: [14, 7, 0] }],
      ["head", 0.92, { rot: [8, 0, 0] }],
      ["head", 1.3, { rot: [0, 0, 0] }],
      ["wing_r", 0, { rot: [0, 0, 0] }],
      ["wing_r", 0.24, { rot: [0, 0, -10] }],
      ["wing_r", 0.58, { rot: [0, 0, -25] }],
      ["wing_r", 0.92, { rot: [0, 0, -12] }],
      ["wing_r", 1.3, { rot: [0, 0, 0] }],
      ["page_r", 0, { rot: [0, 0, 0] }],
      ["page_r", 0.24, { rot: [0, -18, 0] }],
      ["page_r", 0.58, { rot: [0, -150, 0] }],
      ["page_r", 0.92, { rot: [0, -172, 0] }],
      ["page_r", 1.3, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("perch_hop");
});
