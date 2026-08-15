# Vanilla Weather

Topic: vanilla-weather

Status: reference notes for Minecraft Java 1.17.1 vanilla weather. This records
one comparative behavior shape; it is not the implementation plan or
correctness target for Mclone's original atmosphere systems.

## Scope

This topic covers vanilla Overworld weather state, local precipitation, thunder
and lightning, weather randomness, and the notable absence of richer weather
simulation such as wind, tornadoes, and wildfire events.

Minecraft Java 1.17.1 is the specimen documented here, not the project target.

## Vanilla Weather Model

Vanilla has a small global weather state machine:

- clear;
- raining;
- thundering.

Snow is not a separate global weather state. It is the local precipitation
result when the global state is raining and the biome/temperature at a position
is cold enough for snow.

The global state is owned by level data and driven in
`ServerLevel.tick(...)`. The client receives rain and thunder levels as smooth
values rather than only booleans.

Relevant reference files:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/Level.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/commands/WeatherCommand.java`

## Randomness And Seeds

Weather is random runtime state, not terrain-seed-deterministic worldgen.
`Level` owns a `new Random()` instance, and `ServerLevel` uses that runtime RNG
for rain/thunder timer changes and lightning checks.

The current weather booleans and timers are saved in world data, so save/load
continues the active timeline. Two worlds with the same terrain seed should not
be expected to produce the same weather history unless all runtime state and
random draws are also reproduced.

In 1.17.1, the default timer ranges are:

- clear before rain: `random.nextInt(168000) + 12000` ticks, about 10 to
  150 minutes;
- rain duration: `random.nextInt(12000) + 12000` ticks, about 10 to
  20 minutes;
- clear before thunder: `random.nextInt(168000) + 12000` ticks, about 10 to
  150 minutes;
- thunder duration: `random.nextInt(12000) + 3600` ticks, about 3 to
  13 minutes.

Weather cycling only advances when `doWeatherCycle` is true. Commands can set
weather directly through `/weather clear`, `/weather rain`, and
`/weather thunder`.

## Biome And Position Effects

The global raining state does not mean visible rain everywhere. Local
precipitation depends on biome, temperature, and sky access.

`Level.isRainingAt(pos)` returns true only when:

- the level is raining;
- the position can see the sky;
- the motion-blocking heightmap does not cover the position;
- the biome precipitation is rain;
- the biome temperature at the position is at least `0.15`.

Biome precipitation has only three vanilla values:

- `NONE`;
- `RAIN`;
- `SNOW`.

During chunk weather ticks, vanilla can convert a rainy biome's precipitation
to snow if the biome is cold enough at the target position. This is where snow
layers, ice formation, and block precipitation handling fit.

Implication for mclone: model global weather separately from local
precipitation queries. Rendering, block effects, particles, sounds, and
gameplay checks should ask the local biome/position question instead of treating
"raining" as universally visible rain.

## Thunder And Lightning

Thunder is a global state layered over rain. `Level.isThundering()` also checks
dimension constraints and the smoothed thunder level. In the Overworld, storms
darken the world, enable lightning, affect some mob/light checks, and allow
channeling tridents to summon lightning under the right conditions.

Chunk ticks attempt natural lightning while it is raining and thundering:

- per ticking chunk, `random.nextInt(100000) == 0` gates the attempt;
- the target is selected near a random block position in the chunk;
- the final target must satisfy `isRainingAt(...)`.

Lightning can ignite fire around the strike point through the lightning entity.
Because storms usually include rain, exposed fires often extinguish or fail to
spread far, but flammable blocks such as trees can still catch fire depending
on placement and rules. `doFireTick` governs natural fire spread and
extinguishing behavior.

## What Vanilla Does Not Have

Vanilla 1.17.1 does not include gameplay weather systems for:

- wind or wind physics;
- tornadoes, hurricanes, or storm cells;
- humidity fronts, pressure, or seasons;
- wildfire as a distinct weather event;
- hail, sandstorms, blizzards, or fog banks as independent weather systems.

Fog exists as rendering/biome/dimension/effect behavior, not as a member of the
rain/thunder weather cycle.

The mod ecosystem has many examples that expand this space with richer weather,
seasons, ambient particles, wind, tornadoes, biome-specific fog, falling leaves,
clouds, and wildfire-like behavior. Treat those as non-vanilla design
extensions, not parity requirements.

## Mclone Direction

Design Mclone weather from the original product experience. The simple global
cycle and position-local precipitation described above remain useful
comparative mechanisms, but Mclone need not implement them first or preserve a
vanilla contract:

- global weather state and timers belong in shared simulation/server state;
- local precipitation belongs behind biome/temperature/sky-access queries;
- visual rain, snow, thunder darkening, sounds, particles, block ticks,
  lightning, and fire effects should consume those shared queries;
- wind, tornadoes, wildfire events, seasons, and heavy ambient particle systems
  should be explicit Mclone systems with their own contracts, not hidden inside
  retained Java-comparison logic.
