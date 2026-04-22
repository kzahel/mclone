# Oracle Harness

Small Java fixtures live here so the TypeScript port can compare against real Minecraft 1.17.1 behavior.

## PRNG oracle

`pnpm --silent oracle:gen prng --seed 12345 --count 10000 > test/fixtures/prng/seed-12345.json`

The dumper targets `net.minecraft.world.level.levelgen.SimpleRandomSource`, which is the classic 48-bit Java LCG used by Minecraft 1.17.1. In later Mojang mappings, this same legacy generator is named `LegacyRandomSource`.

Use `pnpm --silent` when redirecting to a file; otherwise `pnpm` prepends its own script banner ahead of the JSON stream.

Each output array is generated from a fresh `SimpleRandomSource(seed)` instance:

- `nextInt`: JSON numbers
- `nextLong`: signed decimal strings, to preserve full 64-bit precision
- `nextDouble`: JSON numbers from `Double.toString(...)`

The fixture metadata records the exact class and wire format so the TypeScript side can load longs as `BigInt` or another lossless representation.

## Wrappers

- `oracle/build.sh`: compiles `oracle/java/*.java` into `oracle/classes`
- `oracle/run.sh`: builds if needed, then runs `OracleDumper` with the deobfuscated client jar on the classpath
