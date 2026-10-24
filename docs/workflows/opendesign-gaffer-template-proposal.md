# OpenDesign / Gaffer template proposal

**Author:** Bunsen (OpenCode) · **Reviewed by:** Fozzie · **Date:** 2026-10-23
**Branch:** `docs/bunsen-opendesign-gaffer` · **Base:** `origin/main` @ `2babf5b`
**Status:** proposal only — no application code, no installation, no account or model change.

## Scope

One document, five sections:

1. Verified OpenDesign interfaces for project / file / template access.
2. What we reuse from the already-installed HyperFrames + motion-graphics instead of adding an engine.
3. One proposed Gaffer asset template (artwork).
4. One small proposed Words template (lyrics / titles / timing).
5. Recommendation: reuse skills/templates directly vs. full OpenDesign integration.

Out of scope by instruction: editing Magic Studio code, installing OpenDesign or any CLI, changing Jesse's
chosen runtime/model, generating media, and any broad architecture audit.

Two names from the product vocabulary are used throughout: **Gaffer** is the in-app artist
(Codex/`gpt-6-luna` by the default of `image.generate`), **Sproket** is the conversational coordinator —
that is the spelling in the code (`crates/engine/src/agent.rs`: "Sproket · Magic Studio", "You are
Sproket"), while the task vocabulary spells it *Sproken*. **Words** = approved lyrics/titles/timing.
**Artwork** = drawings/motion design.

---

## 1. Verified OpenDesign interfaces (project / file / template access)

All rows below are **read from upstream source** at `nexu-io/open-design` @ `main`
(cli: `apps/daemon/src/cli.ts`, MCP: `apps/daemon/src/mcp.ts`, docs under `docs/`).
**No OpenDesign command was executed here** — OpenDesign is not installed on this machine, so every
command below is labelled **not run**.

### 1.1 CLI (`od`) — file and project access

| Command (exact, from `cli.ts` usage strings) | Effect | Write scope | Source |
| --- | --- | --- | --- |
| `od files list <projectId>` | list project files | read | `cli.ts` L8997 |
| `od files read <projectId> <relpath>` | read one file | read | `cli.ts` L9014 |
| `od files write <projectId> <relpath> [< stdin]` | write one file | **write** | `cli.ts` L9057 |
| `od files upload <projectId> <localpath> [--as <relpath>]` | upload a local file | **write** | `cli.ts` L9030 |
| `od files delete <projectId> <name> [--json]` | delete a file | **delete** | `cli.ts` L9090 |
| `od files diff <projectId> <relpathA> [<relpathB> \| --against -]` | diff two revisions | read | `cli.ts` L9110 |
| `od files versions <projectId> <relpath>` | list file versions | read | `cli.ts` L9127 |
| `od files version-read\|version-create\|version-restore\|prepare-comments <projectId> <relpath> …` | version history / comment prep | read, **write**, **write** | `cli.ts` L9153–L9216 |
| `od project list\|info\|create\|duplicate\|delete\|import\|import-folder\|open-in\|create-design-system\|share\|revoke-public-link\|restore-automatic-scenario <id>` | project lifecycle | read, **write**, **delete** | `cli.ts` L7442–L7954 |
| `od skills list\|show\|install\|uninstall [<id>]` (alias `od skill install`) | functional-skill registry | read, **write** | `docs/skills-protocol.md` §6 |
| `od plugin list\|search\|info\|manifest\|install\|uninstall\|apply\|diff\|doctor\|verify\|run …` | packaged plugin workflows | read, **write** | `cli.ts` L3120–L3151 |
| `od skill install github:owner/repo` / `https://…/my-skill.tar.gz` | import one `SKILL.md` bundle | **write** | `docs/skills-protocol.md` §6 |
| `od mcp [--daemon-url <url>]` | run the stdio MCP server that proxies tool calls to a running daemon | read+write | `cli.ts` L2729 |
| `od mcp install <agent> [--print\|--dry-run] [--uninstall]` | register the MCP server into a coding agent's own config | **write** (agent config) | `cli.ts` L3016 |
| `od agent setup deepseek-harness` | install/repair a native runtime connection | **write** | README §Platform Compatibility |

`od skill install` accepts `github:owner/repo` or a public HTTPS `.tar.gz`/`.tgz` containing one
top-level `SKILL.md`; downloads are size-capped and reject private-network targets, path traversal,
symlink/hardlink payloads, malformed manifests and duplicate skill ids (`skills-protocol.md` §6).

### 1.2 MCP server — 22 tools

From `apps/daemon/src/mcp.ts` (`TOOL_DEFS`), not run here:

- **Project/file (read):** `list_projects`, `get_project`, `list_files`, `get_file`, `search_files`, `get_artifact`, `get_active_context`, `list_agents`.
- **Project/file (write):** `create_project`, `create_artifact`, `write_file`, `delete_file`, `delete_project`.
- **Catalog / run:** `list_skills`, `list_plugins`, `start_run`, `get_run`, `cancel_run`.
- **Brief UX:** `collect_brief`, `confirm_brief`, `start_vela_login`, `get_vela_login_status`.

`write_file` is documented as the tool for iterating on a file that already exists (no
`ArtifactManifest` required); `get_file` returns text mimes only (HTML, JSX, CSS, JSON, SVG, Markdown)
and errors on binary files — binary assets must go through `list_files` metadata or an upload path.
`delete_project` requires an explicit project id **and** `confirm:true` (irreversible, no active-project
fallback).

### 1.3 Skill / design-template interfaces

- Two registries: functional skills (`skills/`, `/api/skills`) and rendering templates
  (`design-templates/`, `/api/design-templates`). A bundle is a directory with a `SKILL.md`;
  optional `od:` front-matter (`od.mode`, `od.surface`, `od.scenario`, `od.design_system.requires`,
  `od.craft.requires`, `od.preview.type`, `od.example_prompt`) — `docs/skills-protocol.md` §2.
- **Run-time staging:** the daemon copies the active bundle into
  `<project-cwd>/.od-skills/<basename>-<source-path-hash>/` (real copies, not symlinks) and tells the
  agent the CWD-relative path plus an absolute fallback — `skills-protocol.md` §3, `agent-adapters.md` §4.
- **Prompt composition order:** base system prompt → active design system (`DESIGN.md`, `tokens.css`,
  craft references) → active skill body — `skills-protocol.md` §5, `agent-adapters.md` §4.
- **Runtime invocation for us:** OpenDesign launches `opencode run --format json` with the prompt on
  stdin, discovers models via `opencode models`, and resumes with `-s <session-id>` —
  `agent-adapters.md` §5.6. This is OpenDesign's *own* runtime handling; it does not change what Jesse
  runs here.

### 1.4 Limits and boundaries (as documented)

- Daemon binds `127.0.0.1`; LAN exposure needs explicit `OD_BIND_HOST` + `OD_ALLOWED_ORIGINS`.
- Provider base URLs resolving to private/link-local/CGNAT/metadata ranges are blocked by default;
  opt out per host with `OD_ALLOWED_INTERNAL_HOSTS` (exact hostname, no wildcards) — README.
- **Linux:** no prebuilt Linux artifact is published; Linux users must run from source
  (Node `~24`, pnpm `10.33.x`, `corepack enable && pnpm install && pnpm tools-dev run web`).
  Linux release work is tracked upstream as issue #4368 — README, `QUICKSTART.md`.
- **Name collision:** `/usr/bin/od` on this machine is coreutils' octal-dump. On macOS/Linux an
  OpenDesign install shadows it, and `od` may resolve to the wrong binary — README, `QUICKSTART.md`.
- **Roadmap ≠ shipped:** the README's roadmap checkboxes were not used as evidence of capability here;
  every capability claim above is anchored to a file in `main` at the time of reading.

### 1.5 Explicitly unverified

- No `od` command was run. No OpenDesign daemon, project, or template was created.
- Whether `od files write` can target *our* `.magic` project layout is **unknown** — it writes inside an
  OpenDesign project directory, and we have not tested interoperability.
- Whether OpenDesign's own Linux daemon build works on this host is untested (no installation permitted).
- The `od` binary is produced by `pnpm --filter @open-design/daemon build` → `apps/daemon/dist/cli.js`
  (`QUICKSTART.md`); building it was out of scope.

---

## 2. Reuse from the installed HyperFrames / motion-graphics — do not add a second engine

**The OpenDesign "motion" skill is an upstream HyperFrames file.** The skill page
`open-design.ai/skills/motion-graphics/` documents `heygen-com/hyperframes`
`skills/motion-graphics/SKILL.md` (Apache-2.0, recorded commit `558c11b`, snapshot read 2026-09-14).
Upstream's own install line is `npx skills add https://github.com/heygen-com/hyperframes --skill motion-graphics`.
It routes to `/hyperframes` — the same framework and CLI we already render with.

**Already installed here (verified by listing, not by running):** `~/.agents/skills/hyperframes`,
`hyperframes-cli`, `hyperframes-registry`, `media-use`; `~/.hyperframes/` (config, registry cache,
lint streaks). `hyperframes` is not a global binary; the skill drives it through `npx hyperframes`.

**One correction worth stating plainly:** our local `hyperframes` skill set is *our own* entry point
over that upstream project and CLI — it is not a byte copy of upstream's `motion-graphics/SKILL.md`,
and `motion-graphics` itself is not installed locally. So importing OpenDesign's motion skill would
not add a capability; it would add a **second, overlapping entry point to one framework**, with two
routers for the same CLI. That is duplication, which is why the recommendation below is to take
OpenDesign's *reference text* rather than its skill bundle.

**What OpenDesign would actually add (text, not an engine):**

| OpenDesign artefact | Reusable content | Why we want it |
| --- | --- | --- |
| `design-templates/hyperframes/` — `SKILL.md`, `house-style.md`, `visual-styles.md`, `patterns.md` | creative-direction rules, "lazy defaults to question" anti-slop list, colour/typography guidance | house-style rules we can adopt verbatim for Artwork direction |
| `design-templates/hyperframes/palettes/*.md` (bold-energetic, clean-corporate, dark-premium, jewel-rich, monochrome, nature-earth, neon-electric, pastel-soft, warm-editorial) | named, hex-level palettes | a ready source for **locked palette sets** for Gaffer templates |
| `design-templates/hyperframes/references/` (typography, transitions, captions, motion-principles, audio-reactive …) | reference knowledge | reuse as text; our registry already ships equivalents |
| `craft/*.md` (typography, color, anti-ai-slop, accessibility-baseline …) | brand-agnostic craft rules | usable as an Artwork checklist |

**Verdict:** OpenDesign adds **no render engine** we need. HyperFrames remains the only renderer for
Artwork motion; Magic Studio's `mc` engine remains the only project authority. What OpenDesign can
contribute is *catalogue text* — palettes, house style, template workflow — that we copy into our own
skill/template files. That is a file copy, not an integration.

---

## 3. Proposed Gaffer asset template (Artwork)

**Name:** `gaffer-cutout-character` · **Version:** 1 · **Runs through:** existing `image.generate` +
`image.image_size` + `asset.cut_sheet` + `project.add_asset` + `asset.build_cutout_rig` (no new engine).

### 3.1 Reference artwork (input, locked by identity not by pixels)

- Reference PNG (proposed location): `assets/characters/<character>/reference.png` (project-relative),
  with the sha256 recorded in the manifest. A missing reference is a hard error — Gaffer never invents
  a character.
- Optional second reference: the approved turnaround sheet
  `assets/sheets/<character>-turnaround-2048.png`, used only to match palette and line weight.
- Up to 5 references total, matching the existing `image.generate` contract
  (`reference_asset` + `references[]`, max 5 including `reference_asset`).

### 3.2 Outputs — dimensions, alpha, part names

| Output | Dimensions | Alpha | Notes |
| --- | --- | --- | --- |
| Cutout sheet | 2048 × 2048 px, PNG, sRGB | **required** (transparent background) | `image.generate` is asked for `transparent_background=true` (the engine passes the request to the native tool — it is not a guarantee), then the size is normalised with `image.image_size`; the alpha check in §3.6 is what enforces it |
| Pose sheet (optional variant) | 2048 × 2048, 3 × 2 cells of 600 × 900, 48 px gutters | required | same canvas rules as the existing `asset-sheets` skill |
| Parts | cut from the sheet by `asset.cut_sheet` | required | one PNG per part, white background removed |

Part names (a fixed vocabulary; these snake_case tokens are this template's normalisation of the
skeleton-sheet limb list in `.agents/skills/asset-sheets/SKILL.md` — "head, torso, upper arm L/R,
forearm L/R, hand L/R, thigh L/R, shin L/R, foot L/R, tail"):
`head`, `torso`, `upper_arm_l`, `upper_arm_r`, `forearm_l`, `forearm_r`, `hand_l`, `hand_r`,
`thigh_l`, `thigh_r`, `shin_l`, `shin_r`, `foot_l`, `foot_r`, `tail` (optional),
plus `prop_*` for held props. Left/right are the **character's** left/right, never the viewer's.

### 3.3 Locked design constraints (not prompt-only)

| Constraint | Value |
| --- | --- |
| Canvas | 2048 × 2048, PNG, sRGB, integer pixels |
| Background | fully transparent for cutouts; flat `#ffffff` only for reference sheets |
| Clear margin | ≥ 64 px from every canvas edge; ≥ 48 px between cells |
| No text | no lettering, labels, arrows or measurement marks on the sheet (prompts are saved beside the file instead) |
| No shading drift | no gradients, drop shadows or textures in the background |
| Baseline | feet of the front view on the shared baseline; profile views keep the nose pointing right |
| Scale | character height drift ≤ 4 % between views; poses keep the same scale, palette and camera |
| Palette | one named palette, hex-locked, e.g. `ink-warm` = ink `#1b1b1f`, skin `#f2c9a0`, accent `#ff2d55`, shadow `#2a2a33` (illustrative — the real set ships with the template) |
| Materials | flat cel shading, max 3 tones per material, hard colour breaks, no gradients inside the character |
| Identity | same character as the reference — palette, line weight and proportions are not redesigned |

### 3.4 Editable fields (small, by design)

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `character` | slug | — | required; the only identity-bearing field |
| `sheet` | `cutout` \| `poses` | `cutout` | which layout to draw |
| `palette` | palette slug | template default | picks one of the locked palettes; cannot introduce new hexes |
| `poses` | ordered list of ≤ 8 pose names | template default | only when `sheet=poses` |
| `notes` | ≤ 200 chars | empty | one short direction line; never overrides locked constraints |

Everything else (canvas, margins, palette hexes, part names, baseline, shading rules) is **fixed** and
is not exposed as a field. A template that needs a new hex adds a new palette, not a free-text override.

### 3.5 Returned artifact metadata (the contract, not the picture)

One manifest written beside the sheet, e.g. `assets/sheets/<character>-cutout.manifest.json`:

```json
{
  "format": "magicstudio.gaffer.template",
  "version": 1,
  "template": "gaffer-cutout-character",
  "character": "hopper",
  "sheet": "cutout",
  "palette": { "name": "ink-warm", "roles": { "ink": "#1b1b1f", "accent": "#ff2d55" } },
  "source_reference": { "path": "assets/characters/hopper/reference.png", "sha256": "<hex>" },
  "request": { "tool": "image.generate", "model": "<as reported by the tool>", "references": ["<sha256>"] },
  "outputs": [
    { "path": "assets/sheets/hopper-cutout.png", "width": 2048, "height": 2048,
      "alpha": true, "sha256": "<hex>", "background": "transparent" }
  ],
  "rig": {
    "canvas": [2048, 2048],
    "layer_order": ["head", "torso", "upper_arm_l", "upper_arm_r", "forearm_l", "forearm_r",
                    "hand_l", "hand_r", "thigh_l", "thigh_r", "shin_l", "shin_r",
                    "foot_l", "foot_r"],
    "layers": {
      "head": { "box": [768, 96, 512, 480], "pivot": [1024, 336] },
      "torso": { "box": [768, 560, 512, 720], "pivot": [1024, 560] }
    },
    "part_dir": "assets/characters/hopper/parts"
  },
  "validation": {
    "programmatic": {
      "canvas_size": "pass", "alpha_present": "pass", "edge_margin_px": 64,
      "view_count": 4, "max_height_drift_pct": 3.1, "palette_conformance": "pass",
      "part_count": 14, "part_names_match": "pass", "pivots_in_canvas": "pass"
    },
    "visual_review": { "required": true, "reviewer": null, "result": null }
  },
  "created_at": "<iso8601>"
}
```

The `rig` block is deliberately **not** a new format: it mirrors the layered-rig manifest the engine
already loads (`crates/engine/src/session.rs`, `load_keyed_layers`) — `canvas`, `layer_order`, and
`layers[id].box` + `layers[id].pivot`, with one `<id>.png` per layer next to the manifest, alpha
required, dimensions matching the declared box, and the pivot inside the canvas. Layer ids must be
alphanumeric plus `_` and `-`, which the part-name vocabulary already satisfies. A Gaffer result can
therefore feed the existing rig loader instead of inventing a parallel one.

The rest of the manifest mirrors the receipts the engine already writes for registered frames
(`registration.json` with `canvas`, `source_sha256`, pivots) and for cut sheets (manifest with
`part_names`, `frames[].label`, `warnings`, `review`), so a Gaffer result is machine-checkable on the
same terms as the existing PhotoCraft path.

### 3.6 Programmatic validation vs. visual review

**Can be asserted by code (and therefore must be):**

| Check | How |
| --- | --- |
| exact 2048 × 2048, PNG, sRGB | `image.image_size` normalises; assert decoded width/height/colour type |
| alpha present and background really transparent | decode RGBA, assert corner pixels are transparent |
| ≥ 64 px clear margin, nothing clipped | alpha bounding box per cell vs. canvas edges |
| 4 views (cutout/turnaround) or 6 pose cells present, no overlap | connected components per cell + cell-box overlap test |
| height drift ≤ 4 % between views | alpha height of each cell |
| palette conformance | quantise non-transparent pixels, compare to the locked hex set (ΔE threshold) |
| part count and part names | `asset.cut_sheet` manifest (`part_names`, `frames[].label`) vs. the fixed name vocabulary |
| pivot sanity | pivot inside the canvas and inside its own crop box — the rig loader rejects it otherwise |
| layer file shape | one `<id>.png` per layer, alpha present, pixel size equal to the declared `box` |

**Needs human (or Sproket/Gaffer) visual review:**

- character identity and likeness against the reference (same character, not a redesign);
- that the pose reads correctly and the gesture is the requested one;
- profile orientation (nose right) — a silhouette heuristic can flag it, but only review is authoritative;
- line weight consistency and "no AI-slop" quality (the `craft/anti-ai-slop.md` checklist is prose);
- stray lettering or labels baked into the sheet — a text detector can flag a suspicion, but it cannot
  prove the absence of text;
- anything that looks like a hand/face artifact at pixel scale.

**Honest limit:** a prompt can *ask* for a flat `#ffffff` background, a 64 px margin or a hex-locked
palette, but only the checks above make those constraints true. The template therefore records the
manifest and only marks `visual_review.result = "pass"` after a review — it never claims the picture is
correct because the prompt said so.

---

## 4. Proposed Words template (approved words + timestamps, styling separate)

Words already own this separation in the engine: the module contract is that the artist's canonical
lines and the transcript reference supply **text and timing**, while `words.save` edits **only style,
emphasis and motion**. The reference is never edited; mismatches are reported, never substituted
(`crates/engine/src/words.rs`, branch `feature/song-lyrics-reader`; commands `words.read`, `words.align`,
`words.save`, `words.compose`).

The template is therefore a **small, reviewable seed document** — not a re-authoring of the film and not
a new format. It carries the approved words and their timestamps, and leaves styling fields empty so a
person or Sproket can fill them without touching the text.

```json
{
  "format": "magicstudio.words",
  "version": 1,
  "template": "words-section-seed",
  "reference": { "kind": "transcript", "path": "assets/transcript/song-01.json", "sha256": "<hex>" },
  "canonical": { "sheet": "assets/lyrics/hopper-lyrics.md", "original": "assets/lyrics/hopper-lyrics.original.md" },
  "animations": [
    {
      "animation_id": "words-chorus-1",
      "scope": { "kind": "section", "section": "chorus", "occurrence": 1, "line_ids": [], "sung_ids": [] },
      "source_revision": 0,
      "origin": "template:words-section-seed",
      "lead_in": 0.4,
      "hold": 1.0,
      "song_anchor": 41.2,
      "authored_duration": 2.2,
      "style": { "size": 120, "font": "", "color": "", "emphasis_color": "", "outline_color": "", "background": "" },
      "words": [
        { "word_id": "ch1-l1/w0", "text": "we",   "line_id": "ch1-l1", "sung_id": "ch1-l1",
          "ref_index": 118, "start": 41.6, "end": 41.9, "timing_source": "reference",
          "emphasis": false, "action": "none", "entrance": "pop", "exit": "fade" },
        { "word_id": "ch1-l1/w1", "text": "ride", "line_id": "ch1-l1", "sung_id": "ch1-l1",
          "ref_index": 119, "start": 41.9, "end": 42.4, "timing_source": "reference",
          "emphasis": true, "action": "wave", "entrance": "pop", "exit": "fade" }
      ]
    }
  ]
}
```

Rules the template encodes:

- `word_id`, `text`, `start`, `end`, `timing_source` are **copied from the artist's canonical sheet
  aligned to the transcript reference** — never typed fresh, never re-timed.
- `style`, `emphasis`, `action`, `entrance`, `exit` are the **only** fields a template fills with
  content; they are styling/motion and are editable through `words.save` with
  `expected_source_revision` guarding concurrent edits.
- The editable field set is closed and comes from the engine, so a template may not invent one:
  style accepts `font`, `color`, `emphasis_color`, `outline_color`, `background` (strings) and `size`
  (40–320); a word accepts `emphasis` (bool) and `action` (`none|wave|tiny|swim|no|cry|big|float`),
  `entrance` (`pop|fade|rise|none`) and `exit` (`fade|drop|none`). Any other key is rejected by
  `words.save`, including text and timing.
- `mismatches` and `timing_disagreements` are reported by `words.align`; a template must not resolve
  them by inventing text. The engine's own default document carries a `mismatches` array from the
  same alignment — a seed that omits it is still readable, because nothing in `words.read`,
  `words.save` or `words.compose` requires that key.
- `song_anchor` = `first_start − lead_in` and `authored_duration` = `(last_end + hold) − song_anchor`,
  with `lead_in` 0.4 s and `hold` 1.0 s in the engine. In the example above: `41.6 − 0.4 = 41.2` and
  `(42.4 + 1.0) − 41.2 = 2.2`. The audio never moves.
- `reference.sha256` must be the sha256 of the transcript file the words were aligned to; `words.read`
  reports `reference_changed` when the live transcript no longer matches it.
- `canonical.sheet` / `canonical.original` are the artist's sheet and original lyric paths as returned
  by `song.lyrics`; `origin` is a free string identifying where the seed came from (default
  `"magicstudio"`).
- `words.compose` additionally needs the project's HyperFrames composition (`hyperframes/index.html`,
  from `video.init`) and writes only `hyperframes/compositions/words/<animation_id>.html`; the source
  document and the reference stay untouched.
- Template instantiation is *additive*: it seeds one animation per printed section occurrence and never
  rewrites an existing `design/words.json` beyond what `words.save` would accept.

---

## 5. Recommendation

### Option A — Reuse the skills/templates as text (recommended)

Copy the parts we want into our own skill/template files and keep every current runtime:

- adopt OpenDesign's `house-style.md` rules and `palettes/*.md` as *source material* for the Gaffer
  template's locked palette sets and Artwork checklist;
- express the Gaffer and Words templates in our existing `SKILL.md` convention (the same one the
  repo's `.agents/skills/asset-sheets` and `.agents/skills/storyboard` already use) so they are invoked
  by our agents through `mc`, not through a second daemon;
- keep HyperFrames as the only renderer and `mc` as the only project authority;
- change no runtime, no model, no account.

Cost: near zero. Risk: near zero. What we lose: OpenDesign's template *marketplace* and its
plugin/artifact UI — neither of which Magic Studio needs today.

### Option B — Full OpenDesign integration (not recommended now)

`od mcp install opencode` would give our agents OpenDesign's 22 MCP tools (project/file read-write,
`start_run` to a spawned OpenDesign agent, artifact previews). It also brings a daemon to run and keep
alive, a source build on Linux (no prebuilt artifact), an agent-spawning model of its own, and a second
place where project files and prompts live. It would also require deciding who owns the `.magic`
project tree — OpenDesign writes into *its* project directory, and we have not verified any
interoperability with our layout.

We should not take Option B while these are unverified, and it is not needed for the Words/Artwork
split: both halves are already expressible with our existing skills plus HyperFrames.

### Decision requested from Fozzie

Approve Option A. OpenDesign is then a **reference source**, not a dependency: if upstream deletes
`design-templates/hyperframes/`, our copy is unaffected. Nothing in Option A requires Jesse to change a
runtime or a model, and nothing requires a Linux daemon.

### Unverified gaps (explicit)

1. No OpenDesign command was executed — every command in §1 is documented, not run.
2. No interoperability test between an OpenDesign project directory and a `.magic` project.
3. Whether OpenDesign's Linux source build succeeds on this host is unknown (installation out of scope).
4. The Word/Artwork engine contracts cited (`words.*`) live on `feature/song-lyrics-reader`
   (`5c4be84`, `6ddc530`, `3fc6a0d`) and are not yet in `origin/main`; the Words template above follows
   that branch's contract and must be re-checked if that branch changes before the template ships.
5. The Gaffer manifest field `request.model` records what the tool reported; it makes **no** claim about
   relative model quality.
6. The example palette hexes in §3.3 are illustrative until real palettes are adopted.
7. Naming discrepancy to settle separately: the code spells the coordinator **Sproket**
   (`crates/engine/src/agent.rs`, `apps/magicstudio-shell/ui/theatre.js`) while the task vocabulary says
   *Sproken*. This document follows the code; it is not a proposal to rename anything.
8. Our local `hyperframes` skills are our own variant of the upstream project (see §2), so "reuse the
   upstream motion skill" would mean adding a second router for one CLI — the recommendation is
   explicitly to take upstream's reference *text*, not its skill bundle.

---

## Evidence sources

| Claim area | Primary source |
| --- | --- |
| CLI commands and usage strings | `apps/daemon/src/cli.ts` @ `nexu-io/open-design` `main` (fetched 2026-10-23) |
| MCP tool names and semantics | `apps/daemon/src/mcp.ts` (`TOOL_DEFS`) |
| Skill/template format, registries, staging, prompt order | `docs/skills-protocol.md` |
| Runtime invocation, skill staging, security posture | `docs/agent-adapters.md` |
| Linux build, daemon env vars, two execution modes | `QUICKSTART.md` |
| Motion skill is an upstream HyperFrames file (not the same text as our local skill) | `https://open-design.ai/skills/motion-graphics/` (records `heygen-com/hyperframes` `skills/motion-graphics/SKILL.md`, commit `558c11b`, Apache-2.0) |
| Reusable palettes / house style | `design-templates/hyperframes/{SKILL.md,house-style.md,palettes/*.md}` |
| Our own contracts (Gaffer, Words, sheets, rig manifest) | `crates/engine/src/{commands.rs,image_cmds.rs,photocraft.rs,session.rs}` @ `origin/main`; `crates/engine/src/words.rs` @ `feature/song-lyrics-reader`; `.agents/skills/asset-sheets/SKILL.md` |
| Local HyperFrames install | `~/.agents/skills/{hyperframes,hyperframes-cli,hyperframes-registry,media-use}`, `~/.hyperframes/` |

## Verification status of commands cited in this document

| Command family | Run here? |
| --- | --- |
| `od files …`, `od project …`, `od skills …`, `od plugin …`, `od mcp …`, `od agent setup …` | **not run** — OpenDesign is not installed on this host |
| `mc exec image.generate` / `image.image_size` / `asset.cut_sheet` / `project.add_asset` | **not run** — no media generation or application change is in scope for this document |
| `mc exec words.read` / `words.align` / `words.save` | **not run** — the Words engine is not yet on `origin/main` |
