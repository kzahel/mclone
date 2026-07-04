import type { TextureLabApi } from "../../../src/dsl";

const empty16 = "................";

export function defineGrassBlockTextures({
  tint,
  palette,
  texture,
  block,
  macroNoise,
  mask,
  speckles,
}: TextureLabApi): void {
  tint("grass", {
    normal: "#79b34e",
    alternates: ["#5fa343", "#98b85e", "#6fa35b"],
  });

  palette("grass_top", {
    g00: "#5f6848",
    g01: "#68714f",
    g02: "#717a57",
    g03: "#7b845f",
    g04: "#858e68",
    g05: "#8f9871",
    g06: "#99a27a",
    g07: "#a4ac84",
    g08: "#aeb68e",
    g09: "#b8bf99",
    g10: "#c2c9a4",
    g11: "#ccd1ae",
    g12: "#d3d7ba",
    g13: "#dadcc2",
    g14: "#e0e1c8",
    g15: "#e6e5cc",
    g16: "#ebe8cf",
    g17: "#f0ecd3",
    g18: "#f5f0d6",
    g19: "#faf4da",
    base: "#d8dbc5",
    light: "#f1f0cf",
    blade: "#c5cca2",
    shadow: "#969d75",
    dark: "#777f5f",
  });

  palette("grass_side", {
    dirt: "#76533a",
    dirt_warm: "#8c6747",
    dirt_light: "#a37b55",
    dirt_cool: "#604631",
    dirt_dark: "#3b2b20",
    grass: "#cfd8a8",
    grass_shadow: "#98a175",
  });

  palette("grass_overlay", {
    transparent: "#00000000",
    grass: "#f2f5d2cc",
    grass_shadow: "#b9c18a99",
  });

  texture("grass_block_top", {
    size: 64,
    source: "tintable",
    tintRole: "grass",
    palette: "grass_top",
    base: "g13",
    exportPath: "assets/mclone/textures/block/grass_block_top.png",
    catalog: {
      tiling: "xy",
      rotation: "y90-safe",
    },
    layers: [
      /*
       * Diffusion projection provenance:
       * codename: G5101S74
       * candidate: candidate-seed5101-strength0p740
       * raw_candidate_sha256: 75d0ab60f6c6a37596a820eed161d732d7e032fb7eca918cc9479c8ba62531ea
       * diffusion_manifest_sha256: 072e307964c84867ee3ac595515439f738510234f8a6ab9d8c56087e02feb9fc
       * archive_bundle: /tmp/mclone-texture-lab/diffusion-archive/grass-top-g5101s74-active-20-2026-07-04
       * model_id: stable-diffusion-v1-5/stable-diffusion-v1-5
       * prompt_preset: grass-top-tufts
       * prompt: orthographic top-down macro photograph of dense short turf grass, clipped meadow grass blades, small uneven tufts, fine leafy fibers, natural lawn surface, matte organic texture, flat overcast lighting, seamless square material texture
       * negative_prompt: dirt, soil, flowers, clover, broad leaves, long grass, stems, side view, perspective, strong shadows, dew, insects, paths, moss, text, watermark
       * seed: 5101
       * strength: 0.74
       * steps: 24
       * scheduler: PNDMScheduler
       * guidance_scale: 7.5
       * input_sha256: 3f683c4a20f1a3cbf274a3d569c7f8b785112e887beef95086e848d20b5011d3
       * prepared_input_sha256: 3e63b41053ab8c4fad8c2359324f1687b9bb953cb9d6d51f62a39753d1f1df3f
       * preprocess: {"input_grain":0.18,"input_grain_amplitude":18,"input_grain_seed":24680,"pre_blur":10}
       * projection_resolutions: 64
       * projection_palette: g00,g01,g02,g03,g04,g05,g06,g07,g08,g09,g10,g11,g12,g13,g14,g15,g16,g17,g18,g19
       * projection_symbols: abcdefghijklmnopqrst
       * projection: area downsample -> Oklab palette quantize -> deterministic macro majority correction
       * note: exact diffusion regeneration across machines is not guaranteed; the archived raw PNG is the exact proposal artifact.
       */
      mask({
        colors: {
          a: "g00",
          b: "g01",
          c: "g02",
          d: "g03",
          e: "g04",
          f: "g05",
          g: "g06",
          h: "g07",
          i: "g08",
          j: "g09",
          k: "g10",
          l: "g11",
          m: "g12",
          n: "g13",
          o: "g14",
          p: "g15",
          q: "g16",
          r: "g17",
          s: "g18",
          t: "g19",
        },
        opacity: 1,
        upscale: "nearest",
        authoring: {
          role: "structure",
          label: "AUTHOR STRUCTURE MASK",
        },
        pixels: [
          "lnkjinnhgkkkhoqknjklkkmgfijsnlnnkjinnkkkikkohnnnnnkkgggknkkmkjnn",
          "jnllnhjkjhfkkmkknnnnkkkigjhsknnnnnijkkfeikikniknnnkngggomggginnh",
          "nnnnjnjnikkknmkkknnkkhkkskssnnlkknnnkkkklkkklnnnnkknmggojgggnnkn",
          "knnnnnnnlkkjkhkkihnnokmnssssrnjklnnnnoqpnlkknkghnnljgkmlgkggjknn",
          "nokknnnnnnnkmkllkkkhnjjniknnnnnkknkmssssnnnnokhikikkhknkjnnnknii",
          "kmppqnkfdkinkgglnkhknjjnjlnnjnnkkkkmssslnnnnomkkhfkkkhkkgnnnnnnn",
          "nkkkinknknhjgggghkkhiijnnnnknnjnkmokkjmkkinjkikkhikkiijkjiknknnp",
          "okkkinknnnnngggmnokknnnnnnkijkhnknnksksjkkfjkkkiknhkhkkknjnjhjnn",
          "mnnnmknnlkssklnnkkkkkkomkkkknnnnnnlngglginnjnnnnpnghnokknnkkjihn",
          "nnkknnknssslknnnkmmkkhkkkmnognnjlknqgggghnninnnikhknmdddnnlnlnnn",
          "hnnnnllklssmlknklkkmmhhkmkkkijijnnnlmgominnjnnhgkkkkddddnnnnpnnj",
          "knoknlnnlkssninniiikkkkommkinninjnnlmglmnnjnijigkmkkllddjghknnnk",
          "mkkfnfijnnnnnmnoggggggkmnnniokokkkomnljnnnnnsssinjknmnokkkkmnnnn",
          "kkkkijnnnijkonmlmgggggglnjhnookkmknokinnrpnkllhjjlkjkkknnnkgiilk",
          "knmhnnnnpnnlnnnnnpoglggknjjnkmkkqkkkkllnnljkksssnnknnmkkkkhkknnn",
          "kokghnninlnlnnkjgopqlggliinnkjkpkhkknnnnnknnssslnnnnkkkoomkknnlk",
          "knjimgkgkkmomokknnnnnnnnnknnjsssjnnhiinnkkkmpnkkiiknnnnnnknjlgjg",
          "nqnnggkgkkmmnnoknnnkikiknnnjkmsmnnnjifinmkmlkkkhnknnhjinnlnkiggg",
          "nnnjgkgkjkikkkkinifjnklnnnnkssskjnjkjnnnkkkkkiknnnjihnnhnnnlgggg",
          "rnnjgmgkmkkkkkkijniknnnkkkhjlmssnnknnnnkjhkhkkmnnnjnnnhenlnlkjjm",
          "nnninnnjmklnnllklnkjkhhkkkikinkknnnjlglgggmgnnnkkonlsjignknknnnn",
          "nknkngnidddmpnnpnnnjkkknokkmnnnnnnnkllgmmnggnnnnmkkmsssjkjinnnjj",
          "nhknnhgidddlnnnnnqnkkknnkknqnonnnjghggggmlgghjhnklikjssjnknnnkjn",
          "njknnnnidlddkknnlnnnnokkkookklknnnhhgkkglmggghjnkkkkksssknnnljjn",
          "kkmkkomkjinnjnnnnqrnjmlmnnnliinnmkfggnnnnkgkkkmlnjjnnnnjdondkfhk",
          "kknmkokknhhnknnpnnkllssmnnlignnnmkkknnrnnnknmkhknnnnnnnndlddhkhm",
          "kkkfkmfkfgnnnnnnnjjnssssnnlihnnnmmkkkiknjnnnkkkknjgiijnidnmmkkkk",
          "hkdhpkfkjnnnhjkiknnnsssmnnhjihnikkknnkniknknikoonjjnjjnjdddmkokf",
          "jhgjnjnjggmgjjjnppmmmkmmknnnhnnnjlssnnnnnnnnggnlkgjjnnnjhhhnnnki",
          "kknnnnnggopgnnrrpkmkkkkiknnjjnknssssnjiegkjjggngjgggknnnnhnnnlkk",
          "nnnrnknjnnngnnnnkkkmmnpkknnknnhksjlknkjgknnkggomgjggjklknjnnnnnl",
          "nnnnjnnhgmggknnkkkkkkkkknnjgjjnnkjssnknnnnnkgmogggkllnnnniinnnkn",
          "snsnnnihnnhjkmkknjnnknknnknnmggglgkknnnpnmkkkolijknnnnnnnknnkkmo",
          "lsssnmnlqljkkokknnnnnnnnnnnngglomglmnnnnkhiknkkhnjnhnillnnjjknkk",
          "skssnnknpnnnnkkklnnklkkkknkjggnmggkggnnkkkknkkknnniinnkiiinnkmnh",
          "nhnsnnlpnnnnnmppjiikinnnfknkmgmgggggjikikhknkgkknknnnnihnnihkokk",
          "jhnnnnnlmmnmnnkkmddnknnnnnkknnkhgekknjnjhghlhmslknnnkklnllggnnin",
          "hhnninnnkkkmnkkhdkmdinnnnqnkjinnkkkknnkknnnlkslsnnknllnnmlggnnjh",
          "hjnjnlnkkkkmkkekdmmdkkkknnnlnijnmomknnnnpnnnssksnnjjnnnlkjggknni",
          "nnnnlniikkkmkhgkdmddnhnnpnnknnjnnnkkiiknnnknskssnkkknnniggglnknj",
          "omggigjnnnjnnnnjhkkimjssnnjjkknnlgklggggknonnpnmnnnnnnnjkknkkkkk",
          "mkgkjinnjkjnjjjkikkkmkssnkknnnnjggglonmgnnnmnnpmpnnlnnlnkknkkkjn",
          "gkgkgnnnnnnknnnkhkknmsssnnlknnjilmgggogglmnpnnnlqpnnnllnmokkiklk",
          "ggggnnhnnnjginnnmkknkssklnnnnnjkgmggmgnonnlnqnnqnjjjkllnoomklnmk",
          "jhknmkkkkkkkknnnnnnnnnnigggpnnnnnnnnokkkmnkmnnnndddjkknnnnnnnjij",
          "knnnkkkkkiiijkkhkinknnjnolggnnnkfinnkkggkookhihkdddknkknnqnjknkl",
          "nnniknknkkkinhnkkhkniihnmolgijnninnjkknkinkkhlknnldkknnnnnknnnnn",
          "jninnnnpoknqnnnnnnnkgnjngggokiiheejnokonkkkknnnnmddjjknnlkhinhnn",
          "hgjgjnnnjissnjnlnnnkkkkkkjhkhjkkkknnlnnnpmgmnknpkiiikkkknnnnkkss",
          "nninjjjnsssljnnkknnkiiikklkkiknnnnnnknnnkggmjnnnikknmkjkinjjskss",
          "nejninnnssssnknnjnjnmnmkknkkjnnnnknkjnnkgggginpkikkkkihhjnnisjgk",
          "nnnnnnjhkijjjknnjnknlkkkpimknnnnkilnkkknkglgnnpnkkmkmlkkknknsjss",
          "onnmnnlhknnloongjgggijnnkjnnnknkkklknkinnninkpponinnnnnnopnnpmgg",
          "kkkknnnknnnnmgggljgghknnnknljnlnssssnkjnklnkkokkjiinjinnpnnlgngg",
          "kkhknmlmnjnkngggkggkjnnnnnnknnqsssmsnnnnnnkikokkngenghknkknnpggg",
          "jjkkknnnnjiigmmgkgkgnijnnpnjknnnsmsmhijnnnnjkkfgnnnijinnmnnnngnm",
          "nknnnknnkkhklnlknnnjddddokkgkkkkkinjinnnnnnnmggigggmjinnnnknnlkn",
          "nnpnkinjknnknnnqnnnjndddmkkgkmmkinknknnijnniggnmnkgknnnnhhkkjlkn",
          "nnkjkknnnoonnnnjknjnmmmokkkilnkmnlnnnnkhhnnfgijgkgglnnjikkgknmjn",
          "nlklknnnkkkklnnkikkndnndokmkkkimnpnnnnklhnihlgggkgggigjnekkknnnn",
          "nnnnnnnnnnkknkhkgjnnroonsssjjjknijknkkklkkkknljjnknjggkggggghihn",
          "kknnkjnhkkkknkkihnnnmkkkssshinnnnnknmknjmnmpnnmnnijnnkggkmgknhgn",
          "ijnjghjnmkknkkkknnnkkkkkklsjnnnnnqnlkokkkkkknnnnnnnnogkglkglnjnn",
          "nkninnnkkhnmkopqnkikkonkksslknjhnnpnnkikknoiinljjkknggkknggginnn",
        ],
      }),
    ],
  });

  texture("grass_block_side", {
    size: 32,
    source: "final-color",
    palette: "grass_side",
    base: "dirt",
    exportPath: "assets/mclone/textures/block/grass_block_side.png",
    catalog: {
      tiling: "x",
      rotation: "fixed",
    },
    preview: {
      cube: false,
      rotation: false,
      tiling: "x",
    },
    layers: [
      macroNoise({
        seed: "grass-side-dirt-broad-clods",
        frequency: 4,
        octaves: 2,
        colors: ["dirt_dark", "dirt_cool", "dirt", "dirt_warm", "dirt_light"],
        opacity: 0.28,
        contrast: 1.12,
      }),
      speckles({
        seed: "grass-side-dirt-grain",
        density: 0.2,
        colors: ["dirt_cool", "dirt_dark", "dirt_warm"],
        opacity: 0.56,
      }),
      speckles({
        seed: "grass-side-dirt-light",
        density: 0.08,
        colors: ["dirt_light"],
        opacity: 0.42,
      }),
      mask({
        colors: {
          g: "grass",
          s: "grass_shadow",
        },
        opacity: 0.82,
        upscale: "nearest",
        pixels: [
          "gggggggggggggggg",
          "gggsgggsgggsgggs",
          ".gss..gss..gss..",
          "..s....s....s...",
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
        ],
      }),
    ],
  });

  texture("grass_block_side_overlay", {
    size: 32,
    source: "tintable",
    tintRole: "grass",
    palette: "grass_overlay",
    base: "transparent",
    exportPath: "assets/mclone/textures/block/grass_block_side_overlay.png",
    catalog: {
      tiling: "x",
      rotation: "fixed",
    },
    preview: {
      checkerboard: true,
      cube: false,
      rotation: false,
      tiling: "x",
    },
    layers: [
      mask({
        colors: {
          g: "grass",
          s: "grass_shadow",
        },
        upscale: "nearest",
        pixels: [
          "gggggggggggggggg",
          "gggsgggsgggsgggs",
          ".gss..gss..gss..",
          "..s....s....s...",
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
          empty16,
        ],
      }),
    ],
  });

  texture("grass_block_bottom", {
    size: 32,
    source: "final-color",
    palette: "grass_side",
    base: "dirt",
    exportPath: "assets/mclone/textures/block/grass_block_bottom.png",
    catalog: {
      tiling: "xy",
      rotation: "y90-safe",
    },
    layers: [
      macroNoise({
        seed: "grass-bottom-dirt-broad-clods",
        frequency: 4,
        octaves: 2,
        colors: ["dirt_dark", "dirt_cool", "dirt", "dirt_warm", "dirt_light"],
        opacity: 0.32,
        contrast: 1.16,
      }),
      mask({
        colors: {
          c: "dirt_cool",
          w: "dirt_warm",
          s: "dirt_dark",
          l: "dirt_light",
        },
        opacity: 0.2,
        upscale: "smooth",
        pixels: [
          "..c.....w.......",
          ".....s......l...",
          ".w..............",
          ".......cc....s..",
          "...l.......w....",
          "..........s.....",
          "c....w..........",
          "......l.....c...",
          "...s...........w",
          "........cc......",
          ".l.........s....",
          ".....w.........c",
          ".........l......",
          "..c........w....",
          "......s.........",
          "....w......c....",
        ],
      }),
      speckles({
        seed: "grass-bottom-dirt-dark",
        density: 0.22,
        colors: ["dirt_cool", "dirt_dark"],
        opacity: 0.62,
      }),
      speckles({
        seed: "grass-bottom-dirt-warm",
        density: 0.12,
        colors: ["dirt_warm", "dirt_light"],
        opacity: 0.44,
      }),
    ],
  });

  block("grass-block", {
    kind: "cube",
    faces: {
      top: "grass_block_top",
      side: "grass_block_side",
      bottom: "grass_block_bottom",
      overlay: "grass_block_side_overlay",
    },
  });
}
