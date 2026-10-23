# Theatre local integration

The Animation workspace contains the actual Theatre 0.7.0 editor below its
existing canvas. Native sampled motion remains the baseline; Theatre edits a
selected part's additional rotation. Both timelines share seconds.

Build the declared dependencies with `bun run build` in this directory before
building the Tauri app. The generated bundle is embedded with the other UI files.
The current integration is a local evaluation: Theatre edits are not persisted
to `.acproj`, and native engine Undo is not yet connected to Theatre history.

Theatre core is Apache-2.0; Theatre Studio is AGPL-3.0-only. No release bundle
should be distributed before the Studio license implications are resolved.
Upstream: https://www.theatrejs.com/docs/latest/api/studio
