# Ambient companions

Optional ASCII indicators, off by default: **Settings → Ambient ASCII companions**. They are decorations, with no click, navigation, focus or inference behavior. Sproket's home is the chat header; Gaffer's home is the artwork header. Destinations use the actual existing DOM tabs, not additional workflow steps.

Original reusable two-frame data: `apps/magicstudio-shell/ui/companions.frames.json` (version 1, 2fps, maximum 1500ms burst). CLI/TUI clients can read this JSON directly. The browser controller is `ui/companions.js`; it consumes existing command/result notifications, not a polling service.

Read the explanation through the existing live commands:

```sh
mc --bridge PORT exec ui.state
mc --bridge PORT exec ui.feedback
```

Both expose `companions`: enabled/project, Gaffer/Sproket destination, status, command, request ID, reason and target correlation key; also reduced-motion/hidden/animation and bounded dedupe/job counts. This reports observed operations only, never missing steps or an audit. Failed/blocked operations remain pinned until a successful matching target operation resolves them. A chat question does not resolve another operation. Image completion returns Gaffer home.

Status/list/read/navigation noise is ignored. Terminal job transitions are accepted only after a start or running/cancelling state was observed, and repeated terminal receipts are ignored. Dedupe and job history are capped at 64 each and reset with the project. Late results from another project are ignored. Native UI requests retain their transport IDs; the existing private CLI refresh notification carries its request/target identity. CLI errors that never reach an existing frontend notification are not inferred.

Animation uses one bounded timeout chain at 500ms intervals; it ends within 1500ms of a meaningful change. No companion timers run while idle, disabled, hidden or under reduced motion. Readonly snapshots do not animate or poll. Generic fixtures run with `node apps/magicstudio-shell/tests/companions.mjs`.
