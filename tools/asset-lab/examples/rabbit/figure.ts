import { figure } from "../../src/dsl";

// Rabbit: a small, compact, rounded quadruped. A plump egg-shaped body sits in a
// crouched, slightly sat-up posture; the face is short; the front legs are
// short and tucked; the hind legs are big powerful haunches that fold along the
// body and end in large flat back feet (the signature rabbit shape). Two long
// upright ears stand tall off the top of the head -- the single most
// recognizable cue -- and a small fluffy round cotton tail caps the rump.
// White with a fluffy tail is iconic, so the coat is near-white with soft gray
// shading and pink inner ears / nose.
//
// Forward is -Z (head faces -Z), matching the other quadrupeds and the
// locomotion direction [0, 0, -1].
//
// Grounding: the floor sits at the figure's lowest point. The hind feet and the
// front feet are authored so their undersides land at the same y, and the hop
// body-bob is centered so the feet return to that floor at the low point of
// each cycle.
export default figure("rabbit", ({ mat, asciiTexture, part, box, capsule, sphere, walkCycle, bob, contactSwing, followThrough }) => {
  mat("fur", "#f1f0ec");        // near-white coat
  mat("fur_shadow", "#d9d8d2"); // soft gray underside / haunch shading
  mat("fur_light", "#fbfbf9"); // bright belly / cotton tail / cheeks
  mat("ear_inner", "#e7a9b4");  // pink inner ear
  mat("nose", "#e08a98");       // pink twitchy nose
  mat("foot", "#e7e5df");       // big hind feet, faint gray-white
  mat("eye", "#2a2622");        // dark round eye

  // Face: big dark round rabbit eyes set wide, with a small pink nose dot low
  // and center.
  asciiTexture("face", {
    palette: {
      ".": "#f1f0ec",
      "e": "#2a2622",
      "n": "#e08a98",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "........",
      "...nn...",
      "........",
      "........",
    ],
  });

  // Plump egg-shaped body. A sphere, scaled by being slightly taller-than-wide
  // via an overlapping rump sphere behind it, gives the rounded rabbit form.
  // Centered at origin; legs and feet hang below to the floor.
  part("body", sphere({ radius: 0.42, widthSegments: 20, heightSegments: 14, material: "fur" }));
  // Tall rounded rump bulging up and back -- gives the egg/teardrop silhouette
  // and a high haunch arching over the big back legs (a key profile cue).
  part("rump", sphere({ parent: "body", at: [0, 0.14, 0.32], radius: 0.39, widthSegments: 18, heightSegments: 12, material: "fur" }));
  // Lighter belly/chest patch on the underside front.
  part("belly", sphere({ parent: "body", at: [0, -0.16, -0.18], radius: 0.3, widthSegments: 16, heightSegments: 10, material: "fur_light" }));
  // Soft gray shading over the big haunches so the back legs read as bulky
  // muscle. Pushed lower and outward to fill the thigh over each hind foot.
  part("haunch_l", sphere({ parent: "body", at: [-0.3, -0.08, 0.24], radius: 0.29, widthSegments: 14, heightSegments: 10, material: "fur_shadow" }));
  part("haunch_r", sphere({ parent: "body", at: [0.3, -0.08, 0.24], radius: 0.29, widthSegments: 14, heightSegments: 10, material: "fur_shadow" }));

  // Short neck stub blending the head into the chest (no gap up front).
  part("neck", sphere({ parent: "body", at: [0, 0.16, -0.34], radius: 0.24, widthSegments: 14, heightSegments: 10, material: "fur" }));

  // Rounded head, short face, carried up and forward in the sat-up posture.
  part("head", box({
    parent: "neck",
    at: [0, 0.18, -0.22],
    size: [0.4, 0.38, 0.36],
    material: "fur",
    faces: {
      north: { texture: "face" },
    },
  }));
  // Rounding the head corners with a sphere overlay so it isn't a hard cube.
  part("head_round", sphere({ parent: "head", at: [0, 0.02, 0.02], radius: 0.22, widthSegments: 14, heightSegments: 10, material: "fur" }));
  // Short blunt muzzle/cheeks out front.
  part("muzzle", sphere({ parent: "head", at: [0, -0.1, -0.2], radius: 0.15, widthSegments: 14, heightSegments: 10, material: "fur_light" }));
  // Small pink twitchy nose tip.
  part("nose", sphere({ parent: "muzzle", at: [0, 0.03, -0.13], radius: 0.045, material: "nose" }));
  // Round cheek puffs to keep the face short and chubby.
  part("cheek_l", sphere({ parent: "head", at: [-0.18, -0.06, -0.08], radius: 0.11, widthSegments: 12, heightSegments: 8, material: "fur_light" }));
  part("cheek_r", sphere({ parent: "head", at: [0.18, -0.06, -0.08], radius: 0.11, widthSegments: 12, heightSegments: 8, material: "fur_light" }));

  // Signature long upright ears. Tall, slightly fanned outward, standing well
  // above the head. Pivot at the ear base so they can flop/lag with the body
  // bob. Each ear is a fur capsule with a pink inner panel.
  part("ear_l", capsule({
    parent: "head",
    at: [-0.13, 0.5, 0.04],
    rot: [4, 0, -10],
    radius: 0.08,
    length: 0.52,
    capSegments: 4,
    radialSegments: 10,
    material: "fur",
    joint: { pivot: [0, -0.32, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", capsule({
    parent: "head",
    at: [0.13, 0.5, 0.04],
    rot: [4, 0, 10],
    radius: 0.08,
    length: 0.52,
    capSegments: 4,
    radialSegments: 10,
    material: "fur",
    joint: { pivot: [0, -0.32, 0], axis: [1, 0, 0] },
  }));
  // Pink inner-ear panels facing forward (-Z).
  part("ear_l_inner", capsule({ parent: "ear_l", at: [0, 0.04, -0.055], radius: 0.045, length: 0.4, capSegments: 3, radialSegments: 8, material: "ear_inner" }));
  part("ear_r_inner", capsule({ parent: "ear_r", at: [0, 0.04, -0.055], radius: 0.045, length: 0.4, capSegments: 3, radialSegments: 8, material: "ear_inner" }));

  // Short front legs tucked under the chest. length 0.26, pivot at top (+0.13).
  // attach y=-0.34 -> leg top -0.21 (into body underside), bottom -0.47;
  // front paw underside ~ -0.52.
  part("leg_fl", capsule({ parent: "body", at: [-0.16, -0.34, -0.24], radius: 0.07, length: 0.26, material: "fur", joint: { pivot: [0, 0.13, 0], axis: [1, 0, 0] } }));
  part("leg_fr", capsule({ parent: "body", at: [0.16, -0.34, -0.24], radius: 0.07, length: 0.26, material: "fur", joint: { pivot: [0, 0.13, 0], axis: [1, 0, 0] } }));
  // Small front paws.
  part("paw_fl", box({ parent: "leg_fl", at: [0, -0.16, -0.03], size: [0.12, 0.06, 0.16], material: "foot" }));
  part("paw_fr", box({ parent: "leg_fr", at: [0, -0.16, -0.03], size: [0.12, 0.06, 0.16], material: "foot" }));

  // Big powerful hind legs (the signature). A short, thick upper-leg capsule
  // drives the swing from a pivot up at the haunch; a large flat back foot is
  // parented to it lying along the ground, extending forward. length 0.24,
  // pivot at top (+0.12); attach y=-0.34 -> upper-leg top -0.22, bottom -0.46;
  // the foot box sits just under that with its underside at ~ -0.52, matching
  // the front paws so the whole rabbit is grounded level.
  part("leg_bl", capsule({ parent: "body", at: [-0.27, -0.34, 0.22], radius: 0.12, length: 0.24, material: "fur_shadow", joint: { pivot: [0, 0.12, 0], axis: [1, 0, 0] } }));
  part("leg_br", capsule({ parent: "body", at: [0.27, -0.34, 0.22], radius: 0.12, length: 0.24, material: "fur_shadow", joint: { pivot: [0, 0.12, 0], axis: [1, 0, 0] } }));
  // Large flat hind feet lying on the floor, extending well forward (-Z) from
  // the ankle so they poke out past the chest. Long flat back feet are the
  // clearest rabbit cue from the side, so make them bigger than the front paws.
  part("foot_bl", box({ parent: "leg_bl", at: [0, -0.15, -0.16], size: [0.17, 0.08, 0.5], material: "foot" }));
  part("foot_br", box({ parent: "leg_br", at: [0, -0.15, -0.16], size: [0.17, 0.08, 0.5], material: "foot" }));

  // Small fluffy round cotton tail at the back of the rump.
  part("tail", sphere({ parent: "rump", at: [0, 0.04, 0.34], radius: 0.13, widthSegments: 14, heightSegments: 10, material: "fur_light" }));

  // Hop, hand-authored so every foot plants together (a rabbit hop is closer to
  // a four-foot pronk than an alternating bound; synchronizing all four legs is
  // what keeps the feet planted at the low point instead of floating).
  //
  // Phasing is tuned so the grounded contact pose lands at progress 0 (which is
  // the frame the static sheet renders): all four contactSwing legs use the same
  // phase 0.75 with stanceRatio 0.5, so their planted window straddles progress
  // 0 (mid-stance, legs vertical, feet flat). The body bob uses phase 0.5 so the
  // body is at its LOW point at progress 0 and arcs up to its peak at progress
  // 0.5 (mid-swing, feet lifted) -- a clear vertical hop synchronized with the
  // legs. The long ears flop a beat behind the body bob via followThrough.
  //
  // locomotion carries the cycleDistance + forward direction so the review floor
  // scrolls the stride; with one airborne phase per cycle this reads as a hop.
  walkCycle("hop", {
    fps: 24,
    duration: 0.66,
    samples: 21,
    loop: true,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.8,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      // Vertical hop: low (0) at progress 0, peak (2 * amount) at progress 0.5.
      bob("body", { axis: "y", amount: 0.06, center: 0.06, phase: 0.5 }),
      // All four legs swing together and plant together. Stance straddles
      // progress 0 so the feet are flat on the floor at the body's low point.
      contactSwing("leg_fl", { axis: "x", degrees: 24, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_fr", { axis: "x", degrees: 24, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_bl", { axis: "x", degrees: 30, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_br", { axis: "x", degrees: 30, phase: 0.75, stanceRatio: 0.5 }),
      // Long ears flop and overshoot a beat behind the body bob -- the lag is
      // what gives the hop life. Both ears trail together.
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 16, overshoot: 0.85, lag: 0.16 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 16, overshoot: 0.85, lag: 0.16 }),
    ],
  });
});
