# Quest Testbed

## Current contract

Physical Meta Quest operation is delegated to the public sibling repository
`~/code/quest-testbed`. Mclone no longer owns Quest detection, ADB
authorization diagnostics, battery safety, wake/proximity setting snapshots,
system-panel dismissal, interrupted-run recovery, or sleep-after-use.

Mclone retains the parts that describe this product:

- Android and Android XR APK builds;
- Mclone package/activity identifiers and launch-scoped startup argv;
- reference and first-party asset staging;
- app readiness, failure, persistence, multiview, and performance markers;
- dedicated-server startup and project-owned reverse-address policy;
- local WiVRn or Virtual Desktop host/runtime orchestration.

The flat-Quest and standalone Android XR validators enter a transactional
`quest-testbed session` before building and touching the headset. The provider
selects the Quest, exports its serial to the child, records state on-device,
and restores the declared Mclone package, Android settings, proximity sensor,
owned reverse mappings, and sleep state after the validator exits.

Interactive standalone launch uses a detached provider lease because the app
continues after the install script exits. End it explicitly with:

```bash
~/code/quest-testbed/bin/quest end
```

Desktop XR uses the same provider boundary. The macOS WiVRn path holds an
external lease owned by the launcher PID. The Windows Virtual Desktop module
keeps only Streamer service/runtime setup and package-launch policy; Quest
state and recovery are delegated through `bin/quest.ps1`.

## Recovery and evidence

The provider journal lives at
`/data/local/tmp/quest-testbed-session.json`. A normal cleanup removes it only
after critical settings, proximity, and sleep restoration succeed. If a run is
hard-killed or the USB connection disappears during cleanup, run:

```bash
~/code/quest-testbed/bin/quest doctor
~/code/quest-testbed/bin/quest recover
```

Recovery from a different controller requires an explicit `--force` after
confirming the original test is no longer active. This prevents one controller
from silently disrupting another live headset session.

The provider was physically validated on the registered Quest 3 through the
Linux `laptop` controller: ADB selected the Quest while an unrelated Pixel was
also attached, reported an authorized sleeping headset at full USB power, and
found no pre-existing recovery journal or proximity override.
