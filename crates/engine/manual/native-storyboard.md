---
{
  "id": "native-storyboard",
  "kind": "task",
  "category": "storyboard",
  "next": [
    "prepare-art"
  ],
  "status": "researching",
  "verification": "reference"
}
---
# Initialize the editable storyboard

## Input
Project with valid imported timing.

## Command
```sh
mc "$PROJECT" storyboard.init
```

## Output
Native DeckCraft document path.

## Check
Inspect returned deck and existing scene fields. A rendered preview is not evidence of editable objects.
