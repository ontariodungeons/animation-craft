// Companion sprite loader.
//
// The manifest is swappable data: any character may be re-pointed at other
// artwork without touching code. A missing, unreadable or partial manifest
// degrades to the shipped ASCII frames instead of showing nothing, so the
// valid-state fallback is a real fallback and not a decoration.
function createCompanionSprites(manifestUrl = "companions.manifest.json") {
  const manifest = { version: 0, characters: {} };
  const images = new Map();
  let ready = false;

  async function load() {
    try {
      const response = await fetch(manifestUrl);
      if (response.ok) {
        const parsed = await response.json();
        if (parsed && typeof parsed === "object" && parsed.characters) {
          manifest.version = parsed.version || 0;
          manifest.characters = parsed.characters;
        }
      }
    } catch (_) {
      // Fall through to the ASCII fallback.
    }
    ready = true;
    return manifest;
  }

  function character(name) {
    return manifest.characters && manifest.characters[name] ? manifest.characters[name] : null;
  }

  // A pose must exist and be a real file; anything else falls back to rest.
  function poseFor(name, pose) {
    const character_ = character(name);
    if (!character_ || !character_.poses) return null;
    const wanted = character_.poses[pose];
    const fallback = character_.poses.rest;
    const src = wanted || fallback;
    return src ? { src, pose: wanted ? pose : "rest" } : null;
  }

  function image(name, pose) {
    const key = `${name}:${pose}`;
    if (images.has(key)) return images.get(key);
    const entry = poseFor(name, pose);
    if (!entry) return null;
    const img = el("img", { alt: "" });
    img.decoding = "async";
    img.src = entry.src;
    images.set(key, img);
    return img;
  }

  // Whether the artwork for this character is usable at all.
  function has(name) {
    return Boolean(character(name) && poseFor(name, "rest"));
  }

  return {
    manifest,
    load,
    character,
    poseFor,
    image,
    has,
    get ready() {
      return ready;
    },
    // The species/role a character is, as the app already means it.
    role(name) {
      const character_ = character(name);
      return character_ ? character_.role || null : null;
    },
  };
}
