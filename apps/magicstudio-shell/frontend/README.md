# Theatre local integration

The Animation workspace contains the actual Theatre 0.7.0 editor below its
existing canvas. Native sampled motion remains the baseline; Theatre edits a
selected part's additional rotation. Both timelines share seconds.

Build the declared dependencies with `bun run build` in this directory before
building the Tauri app. The generated bundle is embedded with the other UI files.
Save motion stores the native Theatre snapshot in `.magic` and restores it on
reopen. Original sampled motion remains intact. Theatre history and native engine
Undo are still separate. This integration remains a local evaluation.

Bun 1.4.4 rewrites JSZip's optional CommonJS fallback to `__require` without
declaring it. The browser build banner declares that absent fallback explicitly;
JSZip's embedded browser modules supply its actual dependencies.

Theatre core is Apache-2.0; Theatre Studio is AGPL-3.0-only. No release bundle
should be distributed before the Studio license implications are resolved.
Upstream: https://www.theatrejs.com/docs/latest/api/studio

## Learning editor

`bun run build:learning` builds pinned Milkdown Crepe 7.22.3 (MIT) for the existing vanilla shell. No AI feature is enabled. Preserve the upstream MIT notice when distributing the generated bundle: https://github.com/Milkdown/milkdown/blob/main/LICENSE. Task bodies use structured Markdown sections; opaque frontmatter is retained. Unsupported HTML/extensions are refused rather than discarded.

`node --test ../tests/learning-markdown.mjs ../tests/learning-blocks.mjs` checks preservation and caps. From the repository root, `MC_BIN="$PWD/target/debug/mc" node apps/magicstudio-shell/tests/learning-ui.mjs` exercises the real editor through the CLI with temporary project/config fixtures and installed Chromium.
