# Workshop project cards implementation plan

**Goal:** Deliver the six Workshop improvements approved in conversation: responsive project cards, selection-aligned information, compact actions, readable states without paths, stronger MOPS selection, and a studio-wide Workshop header.

**Architecture:** Reuse the observed project catalog, Studio discovery and worker receipts. Keep selection separate from opening Booth. All new buttons use the existing shared mouse/CLI action routing.

**Files:** `apps/mc/src/center/workshop.rs` owns card rendering and selection; `center.rs` integrates layout, header and agent styling; `center/control.rs` and `tui.rs` expose matching keyboard/CLI controls; README documents them.

- [x] Render 1–4 card columns, active first, with collapsible inactive projects, honest unknown states and measured progress only.
- [x] Keep selected preview, details and copied commands tied to the selected catalog project. Clicking selects; Enter/Open observes in Booth.
- [x] Replace the Workshop command panel with compact controls and optional details; preserve the film strips and MOPS bar.
- [x] Add selection styling and fixture checks for wide/compact layouts, scrolling, unavailable observations, identity and navigation.
- [x] Compile, install and reload the existing TUI; inspect its actual rendered frame and CLI controls.

No Desktop app, companion, project-content, or machine-configuration changes. Catalog scope remains the shared recent-project catalog; no project receives invented activity or progress.

Validation: workspace/all-target compilation and the CLI build pass; actual TUI instance 1437626 verified card selection without attachment, details/copy scope, inactive toggles, invalid controls and Open-to-Booth identity. Run the wide/compact fixture checks directly and report the real result. Slow discovery retains one receiver; errors never reuse another project’s facts.
