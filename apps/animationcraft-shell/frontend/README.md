# Theatre local integration

The Animation workspace contains the actual Theatre 0.7.0 editor below its
existing canvas. Native sampled motion remains the baseline; Theatre edits a
selected part's additional rotation. Both timelines share seconds.

Build the declared dependencies with `bun run build` in this directory before
building the Tauri app. The generated bundle is embedded with the other UI files.
Save motion stores the native Theatre snapshot in `.acproj` and restores it on
reopen. Original sampled motion remains intact. Theatre history and native engine
Undo are still separate. This integration remains a local evaluation.

Bun 1.4.4 rewrites JSZip's optional CommonJS fallback to `__require` without
declaring it. The browser build banner declares that absent fallback explicitly;
JSZip's embedded browser modules supply its actual dependencies.

Theatre core is Apache-2.0; Theatre Studio is AGPL-3.0-only. No release bundle
should be distributed before the Studio license implications are resolved.
Upstream: https://www.theatrejs.com/docs/latest/api/studio
