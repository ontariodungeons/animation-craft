---
{
  "id": "choose-audio",
  "kind": "task",
  "category": "audio",
  "next": [
    "timing-skill"
  ],
  "status": "researching",
  "verification": "reference"
}
---
# Use the existing recording

## Input
Resolved $PROJECT.

## Command
```sh
mc "$PROJECT" workshop.files group=audio
```

## Output
Linked original audio path.

## Check
Use the returned existing file. Read supplied intent/lyrics; ask only essential unanswered creative questions.
