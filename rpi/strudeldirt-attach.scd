// StrudelDirt attach-mode startup for the Raspberry Pi systemd service.
//
// The server options + address come from the per-user startup.scd (installed
// from scripts/rpi/startup.scd), which sclang runs first — so `s` already
// targets the systemd scsynth on UDP 57110 with allocators matched to its
// flags. This script just attaches and mounts SuperDirt on UDP 57120; it never
// boots its own scsynth. Run headless by strudeldirt.service via:
//   tail -f /dev/null | sclang scripts/rpi/strudeldirt-attach.scd
// (the `tail` keeps sclang's stdin open so it doesn't quit on EOF — no TTY
// under systemd). Uses the user's default config, so the installed Vowel +
// StrudelDirt quarks are on the class path.
(
s.startAliveThread;

// Wait up to ~20 s (100 ticks x 0.2 s) for scsynth's first /status reply.
s.doWhenBooted({
    s.notify;
    s.sync;
    "scsynth reached + /notify registered (clientID=%, % out buses)"
        .format(s.clientID, s.options.numOutputBusChannels).postln;

    ~dirt = SuperDirt(2, s);
    // Optional sample library — set SC_APP_DIRT_SAMPLES in the service unit
    // (e.g. /path/to/Dirt-Samples/*) to enable sample-based events.
    "SC_APP_DIRT_SAMPLES".getenv !? { |glob| ~dirt.loadSoundFiles(glob) };
    ~dirt.start(57120, 0 ! 12);           // 12 orbits -> bus 0 (stereo)
    "STRUDELDIRT_READY on UDP 57120 (12 orbits)".postln;
}, 100, {
    // Let systemd Restart=on-failure retry until scsynth is reachable.
    "STRUDELDIRT_FAIL: scsynth not reachable on 127.0.0.1:57110".error;
    1.exit;
});
)
