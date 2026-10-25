---
{
  "id": "timing-skill",
  "kind": "task",
  "category": "audio",
  "next": [
    "import-timing"
  ],
  "status": "researching",
  "verification": "reference",
  "blocked": "Exact installed audio-to-lyrics skill entry unresolved"
}
---
# Load the authoritative timing skill

## Input
Original recording and supplied lyrics/timing.

## Source
https://github.com/ontariodungeons/magic-studio/blob/main/README.md#workshop--project-intake

## Output
Exact timing-skill reference, or an explicit blocked state.

## Check
The installed audio-to-lyrics entry is unresolved; audio.analyze is not a confirmed alias.

## Fallback
Ask for the exact skill reference. Preserve existing timed JSON/SRT; do not substitute another ASR pipeline.
