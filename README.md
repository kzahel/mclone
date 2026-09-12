# Mclone

A WIP voxel game and shared Rust engine experiment, with several terrain
generators and experimental level-of-detail systems. There is some exploration,
building, wildlife, and early gameplay, but not much of a game yet.

**[Try it in your browser](https://mclone.kzahel.com/play/)** ·
[Development builds](https://github.com/kzahel/mclone/releases) ·
[Build from source](docs/development.md)

![Mclone Overworld with original assets and experimental distant terrain](https://github.com/kzahel/mclone/releases/download/project-media/mclone-overworld.png?v=8e654fc2)

*An in-game view of Mclone Overworld. Terrain, vegetation, art, performance,
and save formats are all still changing.*

## What is here

Mclone Overworld is the default world generator. Other profiles explore
different terrain ideas, and World Explorer and Terrain Lab provide places
to inspect them. The current terrain LOD uses a shared procedural-horizon
clipmap; it is still experimental and visible seams are possible.

The engine runs across desktop, browser, Android, and OpenXR. Gameplay,
world generation, rendering, persistence, and asset handling live in shared
Rust crates under [`native/`](native/). Platform apps supply their host glue.

## Trying a build

| Target | Entry point |
| --- | --- |
| Browser | [Play](https://mclone.kzahel.com/play/) in a WebGPU-capable browser. This is the easiest way to try it. |
| Linux, Windows, macOS | Extract the matching [development package](https://github.com/kzahel/mclone/releases) and run the application. Keep its resource folder intact. |
| Desktop OpenXR | Use the desktop build with `--desktop-xr` and an installed OpenXR runtime. |
| Android / Quest | Sideload the matching APK; see [installation notes](docs/development.md#desktop-and-headset-builds). |

Nightlies are experimental CI builds, not a promise that every device has
been tested. Desktop downloads are not publisher-signed or notarized.
[Validation notes](docs/tactical/335-public-development-release.md#execution-record)
distinguish compilation from actual browser, desktop, VM, and headset runs.

## Development

The [development guide](docs/development.md) covers fresh setup, build commands,
packaging, and CI. Public builds use Mclone's original and provisional assets;
you do not need Minecraft, its reference files, or extracted assets.

- [Architecture](docs/native-engine-architecture.md) and [platforms](docs/platforms.md)
- [Terrain profiles](docs/topics/world-generation-profiles.md) and [LOD](docs/topics/lod.md)
- [Asset packs and provenance](docs/topics/asset-pack-profiles.md)
- [Contributing and bug reports](CONTRIBUTING.md)

Project licensing is **TBD**. Third-party dependencies and audio retain their
own licenses and notices. This is an independent side project, not an
official Minecraft product.
