import type { TextureLabApi, TextureLayerSpec } from "../../../src/dsl";

export function defineStoneTextures(api: TextureLabApi): void {
  const { palette, texture, block, mask } = api;

  // Value structure follows vanilla 1.17.1 stone (studied locally, not copied):
  // four near-neutral grays in a deliberately narrow luminance band, a dominant
  // mid, a second mid only a hair darker, sparse light, sparse dark pits. The
  // faint cool-green tint is our own; the arrangement below is original.
  palette("stone", {
    pit: "#646664",
    mid: "#717371",
    base: "#7c7f7c",
    light: "#8f928d",
    coal: "#303231",
    coal_light: "#4a4b45",
    coal_dark: "#181a19",
    iron: "#b66d49",
    iron_light: "#d09365",
    iron_shadow: "#7b4e39",
  });

  texture("stone", {
    size: 64,
    source: "final-color",
    palette: "stone",
    base: "base",
    exportPath: "assets/mclone/textures/block/stone.png",
    catalog: {
      tiling: "xy",
      rotation: "y180-safe",
    },
    layers: stoneBaseLayers(api),
  });

  texture("coal_ore", {
    size: 64,
    source: "final-color",
    palette: "stone",
    base: "base",
    exportPath: "assets/mclone/textures/block/coal_ore.png",
    catalog: {
      tiling: "xy",
      rotation: "fixed",
    },
    layers: [
      ...stoneBaseLayers(api),
      mask({
        colors: {
          c: "coal",
          l: "coal_light",
          d: "coal_dark",
        },
        opacity: 0.92,
        upscale: "nearest",
        pixels: [
          "................",
          "................",
          "....c...........",
          "...cdc..........",
          "....d...........",
          "..........cl....",
          ".........cdc....",
          "................",
          "..cl............",
          "..dc............",
          "...........c....",
          "..........cd....",
          ".....cc.........",
          "......d.........",
          "................",
          "................",
        ],
      }),
    ],
  });

  texture("iron_ore", {
    size: 64,
    source: "final-color",
    palette: "stone",
    base: "base",
    exportPath: "assets/mclone/textures/block/iron_ore.png",
    catalog: {
      tiling: "xy",
      rotation: "fixed",
    },
    layers: [
      ...stoneBaseLayers(api),
      mask({
        colors: {
          i: "iron",
          l: "iron_light",
          s: "iron_shadow",
        },
        opacity: 0.9,
        upscale: "nearest",
        pixels: [
          "................",
          "...i............",
          "..sli...........",
          "...i............",
          ".........il.....",
          "........isli....",
          ".........si.....",
          "................",
          ".il.............",
          ".sli............",
          "..s........i....",
          "..........ili...",
          "...........s....",
          ".....il.........",
          ".....s..........",
          "................",
        ],
      }),
    ],
  });

  block("stone", {
    kind: "cube",
    faces: {
      all: "stone",
    },
  });

  block("coal-ore", {
    kind: "cube",
    faces: {
      all: "coal_ore",
    },
  });

  block("iron-ore", {
    kind: "cube",
    faces: {
      all: "iron_ore",
    },
  });
}

function stoneBaseLayers({ mask }: TextureLabApi): TextureLayerSpec[] {
  return [
    /*
     * Diffusion projection provenance:
     * codename: H4106S58
     * candidate: candidate-seed4106-strength0p580
     * raw_candidate_sha256: 4e055ab0f0a5ebe52e1d75d7306d40160d56bc959a80cd32a32c0ae2b1ed9255
     * diffusion_manifest_sha256: 3f238ed9ee04690b8d0f26c40987f489d8d88a1d560644f27ef6b648c122b1d8
     * archive_bundle: /tmp/mclone-texture-lab/diffusion-archive/stone-hewn-h4106s58-active-2026-07-03
     * model_id: stable-diffusion-v1-5/stable-diffusion-v1-5
     * prompt_preset: stone-hewn-horizontal
     * prompt: orthographic top-down macro photograph of rough hewn gray stone surface, horizontal chisel marks, primitive hand tooled rock, shallow layered strata, worn chipped granite, uneven matte stone, subtle pitting and mineral grain, flat overcast lighting, seamless square material texture
     * negative_prompt: perspective, mortar, glossy, colorful, moss, text, watermark
     * seed: 4106
     * strength: 0.58
     * steps: 24
     * scheduler: PNDMScheduler
     * guidance_scale: 7.5
     * input_sha256: 6a28f155576a5fb7e637c1f58c8cb59a1fbc2dc906b763132fc1aecc449e9aab
     * prepared_input_sha256: 3350269417e13b0fa7888071ca93eb21519095e65a348be5e5b0eaa65e8103b3
     * preprocess: {"input_grain":0.16,"input_grain_amplitude":18,"input_grain_seed":12345,"pre_blur":12}
     * projection_resolutions: 64
     * projection_palette: pit,mid,base,light
     * projection_symbols: pmbh
     * projection: area downsample -> Oklab palette quantize -> deterministic macro majority correction
     * note: exact diffusion regeneration across machines is not guaranteed; the archived raw PNG is the exact proposal artifact.
     */
    mask({
      colors: {
        p: "pit",
        m: "mid",
        b: "base",
        h: "light",
      },
      opacity: 1,
      upscale: "nearest",
      authoring: {
        role: "structure",
        label: "AUTHOR STRUCTURE MASK",
      },
      pixels: [
        "mmmmmbbbpppbbhhhhbhhbhhhbbbbbbbbbbbbhhmhmmmmmppbmmmpbbppmmmbbbbb",
        "pmmmmbmmppbbmbhhhbbhhhhbbmbhbbbbbbmbhmhmmmmmppmmmmmmbpppmmbbhbmb",
        "pmmmbmmbbpbbmbhhbbbhhbbhhbbbbbbbbbmmmmhmmmmmpmppmbmmbbbpbbbmmmmm",
        "mmmbbbpbbbpbmbhhbhhhhbbbbbhbbbbbmmpmhhmhbmmbmmppmmmmbbpbmbmmmmmm",
        "pbbbmmmbmmmbbbhhhbbbhhhbbbbhbbbmmmbmmmbmbbmbmmmmmmmmbbmmpppppppp",
        "mmbbbmpmbmmbbbbbbbbbbmbmmmbbbbmmmmbbbbbbbmbmbbmmbbmmmbbbbppppppp",
        "pmbbbbpbbmbmbbbmmbbbmmmmmmmmbmmmmmbbbbbbbbbbbbbbbbbbbbbbbbbppmbm",
        "mpbbbbbbbbbbmmmmbmmmbmmmmmmbmmmmmbmmmmmmmmmmmbbbbbbbbbmbbbbpbbbb",
        "mppmmmmmbbbbmmmbbmmmmmmmmmmmbppbbmmmmmmmmmmbppppmbbbpppbbbbbbbbb",
        "mmmmmmmmbpppmbbbmbbmmmmmmmmbbppbbmmmmmmmbmmmppppmmbmpppbmbbhhhhb",
        "mmmmmmpmbpmmmbbmmbbbmmbbmmbbbbppmmmmmbbmbbmbpbbbmmbbbbppbbbbbbbb",
        "mmmmpmpmbpbbmbbbmmmbbbbmbmbbbpppmmmmmmmmbbbbhhbbbbbbbmpbbbbbbbbb",
        "mmmmmmmmbbbbbbbmmmmmbmbbbbbbbmmbhbhhhhbhhbbbbbbbbbbhbhbhbbbbbbbb",
        "bbmmbbbbbbbbbbmmmmmmmmmbbbbbmbmmhhbhhhbhbbbhhbbbbbbhbhbbbbbmmmmm",
        "mmmbbbbbbbbbbmbbmmmbmmmbbbbbbbmmmhhhbmhhbbbbhhhhbhhhhhhhbbmmmmbb",
        "bbbbhbbbbbbbbbmmmbbbmmbbbbbmbbbbmmbbbbbhbbhhhhhhhhhhbhbhbbbbbmbb",
        "bbhbbbhbbbhhhbbmbmbbbbbbmmmmbbmmmmmbbbmbbhhbmbbbbmbhhhbbhhhhhhhh",
        "bbhbhhbbbbbbbbmmmbbbbmmmmmmmbbbmmmmbbmbbbbbbmbbbbhmhhbbbhbhhhhhh",
        "hhhhhhhbbhbbbbmmmbmmmmmmmmmmmbmmmbbbbbmmmbbbmmmbmmmbbhbbhmmhmmmh",
        "hhhbhhhbbbbbmmmmmmmmmbmmmmmmmbbbbbbbmmbmmmmmmmmmmmmmhhbmmmmmmmmm",
        "hhhhbbbmmbbbmmmmmmmmbbbbmmmmmmmbbbmppmmmmmbbbbmmbbbbbhbmmppppbbp",
        "hhmmmmmmmmmmbmmmmmbbbbbbbhbmbbbbbbbmpmmmbbbbbmbbmmmbbbmmppppppmp",
        "hhbbmmmmmmmmbbbbmbbbbbbbhhhhbbhhbbmpppppmmbmppmbbmmmpmmmppmmppbp",
        "bbhmmbbbbbbbbbbbbbbbbbbbhhhhbbbhbbpmmmpmmbbmbbbmbbbmmmmmbmmmmbbp",
        "bbbbbbbbbbbbbhhhhhhbhhhbbbbhbbbbbmpmmmmmmmbmbbmbmbbmmmmmbmmmbhbb",
        "bbbbbbmmbbbbhhhhhhhhbbbbhhhhbbbmmmmmmmppppmmbbbbbbbbmmmmmmmbhhhh",
        "bmmmmmmmpmmbbbhhhbbhhhhhhhhhbmmmmmmmmmpmmmmmmmbbbbbmmbbbbmmmhhhb",
        "mmppbmbbpppmmbbbbbmbhhbbbbbbmmmbbmppmmmmmmmmmmmmmmmmbbbbmmmmmbhb",
        "mppmmmmmmmmmmmmbbmmmbbbbbmbmmbbbbmmmbmmmmmppppppppppbbbmppppppmm",
        "mmpmmmmbbpppppmmbbbmbmmbbmmbbmbbbbbbmmmmmmmmmmmmpppppmmmmppppppm",
        "bmmppmmmmpppppmmbbbbmpmbmmbbbmbmmmmbmmbbmmmbmmmmmmmpmmbbmmpppppp",
        "mmmmpmmmmmmmmpmmmbmmmmbbmbbbbmmmmmmmmmbbbbbbbbbmmmmmbbbbmmmmmmmm",
        "mbmmpmmppppbbbmbmmmmmmmmmhhhhmmmmmbmmbbbbbbbhhhhbmmbbbbmmmmmbbbb",
        "bbmmmbbmpmmpmmmmmmmbmmmbmmmhmmmhbhbbbbbbbbhbhhhhbmbbhhhbbbbhhhbb",
        "hbbbbbbbbbbbbmmmbbbbbbbbhhhhhmhhhhhhhhhhhhhbbbbbbmbbbhhhhhhhbhhh",
        "bbbmbmbbbbbbbbmbbbbbbbbbhmmmhhhhhhhhbbhhbhhbmmbhbbbbbhhhhhhhhhhh",
        "bbbmmbmbbbmbbhhhbhbbhbhbbbmbhhhhhhhhhhhhbhbbmmbbhhhhhhhbhhhbhhhh",
        "mmbmmmmbbbmbmhhhbbbbbbbmbmmbhbhhhhhhhhhbbbhbbbbbbhhhhhhbhhhmmmhh",
        "bbbbbbbmbbmmmhhhbbmmbbmmmmmbbbhhhhhhhhbbbbbmbmmbmmhhhhbhhhbmmhmm",
        "bmmmbbbmmbmmmbmmmpmmbmmbmbbbbbmbhhhbbbmmmmmmmbmmmmbbbbbbhmbmmhhm",
        "bmmmbmbbbbmbbbmmbmbbbmpmpbbbmmmmmhhbmbbbpmppmmmmmmbmppbbmmbmmmmm",
        "bmmmbmmmbmmmmmmbmbmbbbpmppppbmmmmbhbbbmmmmppmmmmmmmmppppmbmmmmmm",
        "hbmmbbmmmbmmbbmmmmmbbbbmppbpmmmbmmbmbbmmpmmbbbmmmmbbbbppmmmbmbbb",
        "hhbmbbbmbbbbbbbbbmbbmbbbbbbpmmmbmmmmmmmmpppmbbhbbbbbpbbbbbbbbbbb",
        "hbbbbbbmmmhhbbbbbbbbbbbbhbhmhhhmpbbbbmmmmmmbbbhbhbbbbbbbbbbbbbbb",
        "mbbmmbbmmmhhbbbmbmmmmbbbhhmhhhhhmbbbbbbbbbbbhhhhhhhhbbbbbbbbhbbb",
        "mmbmmbbbmbhhbbmmbmmpmmbbhhhhmmmhmmmbbhhhhhbbhhhhhhbbbbbbhbbbbbbb",
        "mbbbmbbbbhhhbmmmbbbmmmmmppmmmmmhmmbbbhbbbbhhbhhbhbhbbbbbbbbbbbbm",
        "bmmbbbbbbbmmmmppmbmmmmmmbbpbpmmbmmmmhhhhbbbbbhbbbbbbbbbbbbbbbbbb",
        "bbmbbbmbbbmmmmmpmmmmmmmmbppppmmmmmmmbhhhbbbbbhbbbpbbbbbbbbbpbbbb",
        "bmmmmmmmbbmmmpppmpppmmmmbbppppmmmmmmmbhhbmbbbbmmppppmbmmppppbbbb",
        "mmmbmmmmmbmmpppmbbmmmmmmbbbmmmppmpmmmmmmmmmbbmmmppppmmmmppppmbbb",
        "bbbhmmmmmmmmmmmmbbmmmmmbmmmmmmppmmmmhhmhbbbbmmmmpmmmmmmmmmmmmmmm",
        "hhbbhmmmmmmmmmmbbbbmbmmbbbmbbbmmhhmmmhmmmmbmmmmmmmmmmpppppmppmmp",
        "bhhhhhbmbbmbhmmbbbmmmbbbbmbmbmbbhhhmhmmmmmbmmmmpmmmpppppmmmmmppp",
        "bhhhhhhmbbbhhbbhbbbmpbbbbbbbbbbbhhhhhhhhbbbmmbmpmppmmmmmmmmmmmpp",
        "hhhhhhbbbbbhbbbbbmbmmmbbhhhhhhhhbbbbmbbbbbmmmmmmmmmmpmmbmmpmbbmp",
        "hhhhbhhbbbbhhhhbbbbmmmbbhhmmmhhhbbmmmmbbbbmmmbmmmbbmmmmmmbmbbbmp",
        "hhbbhhhbbbbbhhbhbmmmmmmmmhmmmmmhbbbmmmbbbbmmbbbbbbbmbbbbbbbmbbbm",
        "hhbbhhbbbbhbbbbhbmmmmmmmhhmmhmmmmmmmmmbbbbhbbbbbbbbbbbbbbbbbbmbm",
        "hbbbbbbbbbbbmhhhbbmpmbbbmmmmbbmmbbmbmmbbbbbbbbbbbbbbbbbmmmmmmmmb",
        "bbbbbbbbbbbmmmhmmbmmbbbbmmmbbhbmmmmbbbmmbbbbbbbmmbbbbbbbhhhhbpmm",
        "bbmmmbmmmmmmhmmhmmbbbbbbmmbbhbbmmmmbbbmmmmmbmpmmmmbbmbmbhhhmmbbb",
        "mmmmmmmmmmmmmhhhbbbbbbbbhhbbbbbbbbbbbmmmmmmmbbmmmppmmmpmmmhhbbbb",
      ],
    }),
  ];
}
