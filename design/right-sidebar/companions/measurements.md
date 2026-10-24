# Native measurement notes

These are short single process-tree samples, not statistically controlled performance claims. CPU includes the owned shell and its WebKit child processes, measured from /proc ticks. Tiling/window size and ordinary app work varied; the final Settings read also completed three non-companion DOM mutations. Do not attribute all whole-app CPU differences to the indicators. All raw JSON samples are retained.

| Sample | Seconds | CPU seconds | % one core | All DOM mutations | Companion frame mutations |
|---|---:|---:|---:|---:|---:|
| baseline-idle | 8.021 | 0.050 | 0.623 | 0 | 0 |
| baseline-operation | 2.055 | 0.020 | 0.973 | 26 | 0 |
| feature-off-idle | 8.020 | 0.060 | 0.748 | 0 | 0 |
| feature-enabled-idle | 8.013 | 0.070 | 0.874 | 3 | 0 |
| feature-burst | 2.056 | 0.040 | 1.945 | 32 | 4 |
| feature-completion-tail | 8.012 | 0.130 | 1.623 | 4 | 4 |
| feature-idle-after-operations | 8.020 | 0.490 | 6.110 | 0 | 0 |
| final-off-idle | 8.021 | 0.110 | 1.371 | 0 | 0 |
| final-enabled-idle | 8.021 | 0.150 | 1.870 | 3 | 0 |
| final-burst | 2.055 | 0.060 | 2.919 | 32 | 4 |
| final-settled-idle | 8.022 | 0.070 | 0.873 | 0 | 0 |

The original empty-study baseline/initial feature samples used the same empty-project workflow. Later samples include one registered existing screenshot asset and window tiling changed. The completion-tail sample includes the tail of the completion burst and is deliberately not labelled idle. One subsequent whole-app idle sample was6.11% of one core despite zero companion timers/DOM writes. The final settled sample was0.873% with zero DOM writes. This variance prevents a causal CPU-overhead claim.

Feature-specific observations: off and settled idle have zero companion frame writes and animating:false; one ordinary project.save burst produced four companion frame writes and had stopped by the2-second readback. The generic virtual-clock fixture verifies >=500ms between frames and no timers after1500ms, including rapid meaningful events. Hidden/off/reduced-motion and polling/no-layout-read cases are fixture-tested; native off and idle were also measured. No native reduced-motion preference was changed.

Native behavior: real failed import -> Sproket at Animation; real reads do not move it; same-path successful retry -> chat home. Existing artwork completion handler -> Gaffer artwork home. This was not native image generation. ui.state and ui.feedback companion fields match; CLI request ID matched last_command.request_id. See native-behavior.json and final-native-layout.json.

Screenshots reviewed: homes.png, failed-operation.png, resolved-and-artwork-home.png and setting-enabled.png. The final Settings capture is a native725px-wide owned floating window; earlier captures1261px. A temporary319px tiled window was below the supported~700px layout range; not claimed as a passing layout. Only the owned window PID was floated for inspection; Fozzie19927 never accessed.

The existing close guard correctly refused close while Settings model reads were pending; after ui.feedback showed ready/no unsaved fields the same owned app quit normally. No force kill. Setting restored off; no production project/worker/native image-generation operations.
