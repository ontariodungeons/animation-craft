# Grok plan — recovered source snapshot

Historical plan, preserved verbatim below; its backend conclusions still require the multi-subject comparison. See track15-animation-sequence.md for the current execution plan.

# Image to animation: four commands on the path that already moved

## The surface that worked was Blender's

Two different Blender hooks are in the tree. They are not the same thing.

The one in the Animation tab today is a headless job. `blender.start` launches `blender --background` (`crates/engine/src/blender.rs`) and the dialog plays the finished `preview.webm`. The window never opens. The Oct 23 note on that worker says the parts are flat planes with rigid rotations, no armature, so antennae do not bend there.

The one that matches the memory is the live session. `pipeline/blender53_bridge.py` sends a Python script to Blender on `127.0.0.1:9876` (`send_blender_mcp_code`) and `execute_archetype_in_blender53` builds the scene in that open Blender, keys it, and saves a `.blend`. That is Blender's own viewport and Blender's own timeline, with the other tools chained after it. The shell's Blender button does not call this. That is the missing surface.

The custom canvas is still there (`#motion-canvas` drawing `motion.sample`). It is the stage in the layout below. It is not a second timeline.

## Two timelines, and the picture to decide from

There is a broken one and a restyled one. They are both already under the Animation stage. Neither is EffectCraft.

- **Keys** is Theatre 0.7 inside an iframe (`theatre.js`, `theatre-frame.html`). That is the broken timeline. It stays mounted even on the other view so its save guard keeps working. Do not restyle it again.
- **Curves** is the restyle (`timeline.js`). It draws each moving channel from the engine sample, in the shell's own colors. Default view is Curves. It is not Blender's dope sheet, and it is not a Blender package.

The Blender approach, as already drawn, is this frame:

![Blender mode in Magic Studio](design/right-sidebar/exports/animation/animation-1261-820.png)

Blender, Legacy, and Composite are the three modes. Blender is selected. The source is "Saved Blender scene". The stage shows the character, the scrubber sits under it, and the left column is object, action, length, and render. Legacy is the old editor. Composite is images arranged in time. That is the layout to keep.

Blender the application is not running, so its own dope sheet could not be captured. Do not draw a new one. The screenshot is the decision picture.

EffectCraft's timeline does not drop in. It is the other app's editor, not a set of components this shell can mount and have update. Reskinning it here would be a third timeline next to Keys and Curves. The reskin that already exists is the frame above.

Refactor toward that frame. Do not add a timeline. Curves stays the channel strip because it already reads the sample. Keys is the duplicate to retire once its save no longer depends on the iframe. Opening Blender is for editing the saved scene, and the stage plays that scene back.

Reimplement the live step, and nothing else around it. One command sends the wired character into an open Blender and leaves that window up to scrub. If nothing is listening on 9876, start Blender with a window, then send the script. Do not use `--background` for this command. Do not bring back the bridge's known mistakes: it always builds a humanoid, it can fall back to Hopper, and a review already recorded that. The script it sends is the character just wired, with that kind's part names. The headless worker stays the batch renderer it already is.

## What changed in this plan

The idle-only cutout shortcut is dropped. A still becomes animation only by drawing the sheets, cutting them, and wiring named parts. That path has already run on this machine far enough to show face, wings, arms, and antennae bending and swaying on idle, at about 80%. Three later attempts did not. This plan finds that working path and wraps it. It does not invent a fourth way.

EffectCraft stays a review comp after a render. It is not the surface to bring back.

## Find the proof before writing commands

`apps/animationcraft` is already gone from this tree. The rename to Magic Studio is commit `52607fd`. The old name only remains as a label in chat history.

Search for the Blender scene that played, not for a canvas recording:

1. Projects the shell already names, starting with `/run/media/jesse/Speedy/media/merriweather/bumble-bee/Bumble Bee.magic`, and the `.blend` files beside them.
2. Other `.magic` projects under `/run/media/jesse/Speedy/media` whose rig or blend lists `wing` or `antenna`.
3. If none of those open in Blender and play, the pre-rename AnimationCraft checkout, only to see which project and which script it called. Do not revive that app.

Open that `.blend` in Blender and scrub. Record which of face, wings, arms, and antennae actually move. A file on disk is not that test. If the 80% is not in the Blender window, say so and stop. The reimplementation is the live send step above. It is not a new renderer.

## The four commands

Each one prints the next command. `mc exec` does not keep a session, so every command takes the project and reopens it. `-y` answers the wizard's questions with the defaults. Without `-y` the wizard asks and waits.

1. `mc animate <image> [--kind humanoid|bee|fish|flower|custom] [-y]`
   - Choose the still and the kind. Default kind is `humanoid`.
   - Locate the sheet templates that already exist. Do not write new template scripts.
   - Ask one question: draw the sheets with Gaffer? Enter and `-y` both mean yes.
   - Gaffer is the artwork companion already in the shell (`companions.js`: artwork role). "Ask Gaffer" runs the existing `assets.agent_sheets`. It does not add an agent. Renaming Gaffer is separate and is not in this plan.
   - On yes, the command prints the exact `assets.agent_sheets` line (kinds `turnaround`, `skeleton`, `poses`) and, beside it, the `image.generate` line if a person would rather draw the sheets themselves. It does not draw until that line is run.

2. Draw. This is the existing command, not a new one.
   - `assets.agent_sheets` asks the configured agent (codex, claude, or omp) and checks the sheets.
   - Or the person runs the printed `image.generate` line.
   - Sheets land in `assets/sheets/<character>-turnaround.png`, `-skeleton.png`, `-poses.png`, each with its `.prompt.md`.
   - The skeleton sheet is the joints and the separated limbs together (`.agents/skills/asset-sheets/SKILL.md`). A fourth joints-only sheet is not added.

3. `mc animate cut project=<file>.magic character=<name>`
   - For each sheet that arrived: flatten the background and make it transparent, then cut and crop each cell to the drawing.
   - Cutting stays `asset.cut_sheet` (PhotoCraft underneath). Cropping uses the existing break-apart / image-size path. No new matte tool.
   - Reply with the cleaned files, one set per sheet, and the names still missing. That set is what gets wired. A file existing is not a pass. Each output is a decoded PNG with transparent outside pixels.

4. `mc animate show project=<file>.magic character=<name> --kind <kind>`
   - Match cleaned files to the kind's part names and build the rig with `asset.build_cutout_rig`.
   - Send that rig into the open Blender, using the same socket the bridge already uses. The Blender window stays open on that scene.
   - The custom stage may show the same subject. The pass is the Blender viewport, where the parts can be scrubbed.

After show, look at Blender. Each Magic Studio control light stays what `companion-boxes.js` already does: green when a real command or sample says the control is in use or ready, yellow when it needs a person, red only on a failed event. An empty control stays unlit. It is not an error.

## Part names, without a class tree

Ponytail applies. The cutter already has one list, `PARTS` in `pipeline/pose_and_part_cutter.py` (head, ears, torso, arm_L, arm_R, leg_L, leg_R, prop_backpack), and the exploded template is that same humanoid grid. A bee, a fish, and a flower are more rows in that table, not subclasses.

- `humanoid` keeps today's names.
- `bee` adds the names the proven rig actually used (wings, antennae, and the face parts that moved). Copy those names from the project found above. Do not invent a bee skeleton first.
- `fish` and `flower` get a short row only after one of each has been cut and played. Until then the wizard accepts the kind and says the name list is not in the table yet.
- `custom` asks for a name list and stores it next to the others. A swaying flower and a bee both enter as a kind with limbs. They do not get their own code path.

One function reads the table and cuts those names. Adding a kind is a row.

## What this does not build

- A new timeline, an EffectCraft timeline mount, or a new renderer. The Blender mode in the screenshot is the player. EffectCraft stays a review comp after a render.
- A fourth sheet, a joints-only sheet, or a new image model.
- Quality levels `xhigh|high|medium|low`. `image.generate` still accepts only `quality=auto`.
- A class hierarchy for asset types.
- Theatre key editing. Playback of this path is the Blender window.

## Verify by opening subjects

The acceptance test is the open Animation view, one subject at a time.

For the proven subject, then for each new still the person supplies:

1. Run the four commands with `-y` where the defaults are right.
2. Open the project and the Blender window. The subject is in that viewport, not only on the canvas.
3. Scrub idle in Blender. Record face, wings or fins, arms or petals, and antennae: moving, present but still, or absent.
4. Read the control lights. Green and yellow must match a real event. A dark control is not a failure.
5. Write that row down before the next subject. A later subject does not get a kind row until this one has a row.

Commands are checked by those pixels and lights. `mc doctor` stays the process check it already is. It does not become the animation test.
