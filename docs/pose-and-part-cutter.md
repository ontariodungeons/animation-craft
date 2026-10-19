# Pose and part cutter

Dependencies: Python 3, Pillow, NumPy and SciPy. Local Tesseract is optional for labels on unknown sheets. No image generation, external service, Blender or FilmCraft is invoked.

Export the supplied vault sheets:

```bash
python3 studio_pipeline_cli.py cut-sheet \
  '/home/jesse/main-vault/Creative/Design/Characters/3d poses' \
  --out-dir exports/pose_and_parts
```

Export one unknown sheet:

```bash
python3 -m pipeline.pose_and_part_cutter my_sheet.jpeg \
  --kind action --out-dir exports/my_character
```

Kinds are `auto`, `action`, `turnaround`, `single`, `exploded`. Auto recognizes the vault filenames; specify the kind for other filenames. Known sheets use source-hash-checked profiles in `exports/templates/vault_sheet_layouts.json`. Unknown sheets use connected components and optional caption OCR. Review frame labels before use, especially for captions above characters or decorative panels. `--no-ocr` disables OCR and leaves unknown poses numbered.

Each folder contains:

- `manifest.json`: source coordinates, actual labels, action lookup, atlas rectangles/UVs, shared dimensions and anchors, part paths, parent graph, pivots, bind positions and review status.
- `frames/`: aligned transparent PNGs at original scale. Select one action; these are not a continuous walk/run cycle.
- `atlas.png`: PNG sprite atlas with 8px per-frame padding.
- `parts/`: trimmed visible-pixel cutouts. Missing or occluded prop artwork is explicitly unavailable.
- `contact_sheet.jpg`: quick visual check on grey.
- `layout.review.json`: editable source-pixel boxes, part polygons and joint pivots.
- `runtime.mjs`: p5.js frame/puppet drawing and Three.js frame/puppet creation.

The manifest is authoritative. Rerunning in an existing folder can leave superseded files with old labels; use a fresh directory for a distributable asset pack.

## Refine a cut

Edit `layout.review.json` and rerun against the original image:

```bash
python3 studio_pipeline_cli.py cut-sheet my_sheet.jpeg \
  --kind action --layout exports/my_character/layout.review.json \
  --out-dir exports/my_character_reviewed
```

All boxes, polygons and `sourcePivot` values use source pixels with a top-left origin. Boxes have exclusive right/bottom edges. Part polygons and their `pivot` in the input layout are in source coordinates. Output `pivot` is measured from the trimmed part's top-left corner; `bindPosition` is measured on the aligned frame canvas. Anatomical L/R is used: in a front view, left is the viewer's right. Automatic partitions assume front view; manually correct side/back views and non-biped anatomy.

A frame definition can supply a part mask and source-space exclusions:

```json
{
  "frames": [{
    "label": "standing",
    "box": [50, 90, 250, 390],
    "excludePolygons": [[[50, 375], [250, 375], [250, 390], [50, 390]]],
    "parts": {
      "head": {
        "polygon": [[80, 140], [220, 140], [220, 230], [80, 230]],
        "pivot": [150, 225],
        "complete": false
      }
    }
  }]
}
```

Use exclusions to remove captions, ground patches or background pockets. Adjust polygons to follow anatomy rather than slicing sleeves or faces. `complete: true` is an explicit user assertion that hidden surfaces/joint overlaps exist and the part is visually reviewed. Remove `inferred: true` when replacing an inferred mask. A single approved part does not approve the entire sheet. Frame/overall quality remains review-required; there is no fabricated model QA score.

Default shared alignment centers frames horizontally and aligns bottoms. It preserves pixel scale, not world-space motion. Optional per-frame `offset: [x,y]` changes placement within the shared canvas and is rejected if it clips. Author temporal frames or interpolate a reviewed puppet for smooth animation.

## Three.js and p5.js

Serve the export folder with your app so image URLs and ES modules resolve over HTTP. Supply your app's existing Three.js/p5.js dependency.

```js
import {loadManifest, threeFrame, threePuppet, p5Frame, p5Puppet} from './runtime.mjs';
const manifest = await loadManifest('./manifest.json');
const atlas = await new THREE.TextureLoader().loadAsync(new URL('atlas.png', manifest.baseURL).href);
atlas.colorSpace = THREE.SRGBColorSpace;
const actor = threeFrame(THREE, atlas, manifest, manifest.animation.actions.standing[0]);
scene.add(actor); // pixel units, feet at origin; use an appropriate orthographic camera
```

For p5.js preload `atlas.png` using `loadImage`, then call `p5Frame(p, atlas, manifest, index, x, y)` inside `draw`. It crops the correct atlas rectangle and uses the shared foot anchor. Do not loop across all named actions as if they were consecutive cycle frames.

For a reviewed layered frame, load each available `frame.parts[name].file` into a texture/image dictionary. `threePuppet(THREE, textures, frame)` returns a group with `userData.joints[name]`; rotate `.rotation.z` in radians (positive counterclockwise). `p5Puppet(p, images, frame, {head: 0.1, arm_L: -0.2}, x, y)` uses radians clockwise in image space. Both implement the part parent hierarchy and bind-pivot offsets. Inferred parts often reveal seams when rotated; use complete exploded artwork for production puppet animation.

## Exact new exploded-sheet prompt

```bash
python3 studio_pipeline_cli.py cut-sheet --prompt Hopper \
  --description 'cream felt rabbit, orange vest, denim shorts, blue sneakers, olive backpack'
```

The exact template is `exports/templates/exploded_part_sheet_spec.json`. It specifies a 3072×3072 sheet, a 3×3 grid, eight named slots, a blank ninth slot, anatomical L/R, common scale, transparent/chroma-key background and painted hidden joint caps. Attach the original reference when using the prompt with an image generator. This command prints the prompt; it does not generate or charge for an image.

Cut that new image with `--kind exploded`. `manifest.puppet` contains the eight part assets and initial estimated assembled bind positions. Optional ears/prop slots may be empty. Edit the layout's `puppet` map using per-part `pivot`, `bindPosition` and `complete` to establish an approved bind pose, then rerun. The initial estimated assembly always needs review.
