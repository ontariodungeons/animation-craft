# Design page (lyrics): handoff brief

Spike: `spike.html` (throwaway reference, not product code). Data: `files/data.json` (Hey Ho Little Fish's 113 lyric clips, read from its `hyperframes/index.html` and `film-data.js`).

To run it, put two files beside the data, then serve this folder (`python3 -m http.server`) and open `spike.html`:
- `files/song.wav` ← `/run/media/jesse/Speedy/media/hey-ho-little-fish/project/hyperframes/track-03.wav`
- `files/backdrop_v1.png` ← `…/hey-ho-little-fish/project/hyperframes/assets/stage/backdrop_v1.png`

## Agreed with Jesse (2026-10-24)
- **One vertical song timeline down the page.** The lyrics are on the left, a band per section and the words of each sung line at its time. A film roll is on the right at the same seconds-to-pixels scale, so each sung line sits level with its frame.
- **The centre stays pinned while you scroll:** the stage and its play slider, then the section's **minimal lanes** (Text, Motion; Drawing and Background are only mocked), then the selected lane's effect cards.
- **The lanes only show timing.** The Studio owns clip length and timing. This page never drags or resizes a clip, and there's no second, complex timeline.
- **Effects are cards.** A card shows its name, a one-line description and its steps. Word options (animation, comes in, goes out) are card libraries too, not small buttons or dropdowns.
- **Lyric tags** (Jesse preferred this version over a plain lyric sheet): no fill, soft square corners, more spacing, and words laid on a grid so they line up. Each tag carries its data (its move) as small text.
- **Navigation:** pick *where* on the left (a section, a line or a word), and the centre shows the options for that pick. One line says what's selected, for example "Changing: Chorus 1 › whale".
- Design shows no Parts & assets pane.

## Already in code
`cfabfb7` on `feature/lyrics-design-timeline` (base `5ab274ba`) ports the layout into `ui/words.js` and the Words block of `ui/style.css`. That covers the vertical timeline, the film roll, the pinned stage, the Text and Motion lanes, the HyperFrames text-effect cards, and the word card libraries. The tag polish above isn't in that commit yet.

## Only mocked in the spike
The Drawing and Background lanes and their effects (Sand sketch, Bubble trail, Tide wash, Night sea), and the "+ add track" button. Nothing renders these in the app yet.

## Related
A second spike for real lyric *creative* (typography, composition and drawn strokes, imported from the Pterodactyl Song's Remotion lyric scenes) is being iterated with Jesse separately. It isn't part of this handoff.
