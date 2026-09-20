// sc-app2 / Raspberry Pi — per-user SuperCollider startup.
//
// Install to ~/.config/SuperCollider/startup.scd. sclang runs this on every
// launch, BEFORE any script passed on the command line, so `s` (Server.default)
// is configured to ATTACH to the systemd scsynth on UDP 57110 by default.
// Never `s.boot` from sclang — scsynth.service owns the server's lifecycle;
// attach with `s.startAliveThread` / `s.doWhenBooted` instead.
//
// These options MUST match scsynth.service's flags:
//   -u 57110 -i 8 -o 8 -a 8192 -b 262144 -m 262144 -w 2048 -n 32768 -l 32
// In attach mode sclang doesn't push them to scsynth, but they still drive
// sclang's LOCAL bus/buffer allocators — a mismatch hands out private audio
// buses that overlap the real hardware outs, or fails buffer allocation.
(
s.options.numInputBusChannels  = 8;       // -i 8
s.options.numOutputBusChannels = 8;       // -o 8
s.options.numAudioBusChannels  = 8192;    // -a 8192
s.options.numBuffers           = 262144;  // -b 262144
s.options.memSize              = 262144;  // -m 262144 (KB)
s.options.numWireBufs          = 2048;    // -w 2048
s.options.maxNodes             = 32768;   // -n 32768
s.options.maxLogins            = 32;      // -l 32
s.newAllocators;

s.addr = NetAddr("127.0.0.1", 57110);     // scsynth.service's port
)
