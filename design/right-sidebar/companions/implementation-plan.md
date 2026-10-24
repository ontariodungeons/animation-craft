# Ambient companions implementation plan (#23)

Approved scope: optional/off-default noninteractive ASCII Sproket and Gaffer; shared original JSON frames; existing settings/signals/state only; <=2fps, <=1.5s bursts, zero idle/hidden/off/reduced-motion timers. No native generation, production writes, audit, dependency or editor.

1. Add generic controller fixtures to existing hook-run UI harness. RED before controller: polling noise; image completion home; persistent failed/blocked operation until matching resolution; stale project; bounded dedupe; timer lifecycle; noninteractive DOM. Native boundaries synthetic.
2. Add small controller/frame JSON and reserved decorative slots using real DOM tabs and chat/artwork homes. Feed captured invoke and CLI refresh identities/results, not heartbeat/navigation. Correlate terminal jobs with observed starts; retain bounded state. Add settings checkbox and readonly UI/feedback state.
3. GREEN focused tests, generic UI checks, workspace compilation and normal commit hooks. Native preview from own worktree, isolated generic study; compare base versus off/idle/burst process-tree CPU and companion DOM mutations. Store all measurements/screenshots here, close owned app. Commit explicit source paths and hand off; no integration/push/deploy.

Files: ui/companions.js, ui/companions.frames.json, ui/app.js, ui/bridge.js, ui/feedback.js, ui/index.html, ui/style.css; tests/companions.mjs and hook-run tests/agent-selection.mjs import. No backend poller. Readonly feedback may require preserving this field through existing Rust feedback snapshot (inspect first).
