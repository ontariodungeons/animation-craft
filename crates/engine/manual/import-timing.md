---
{
  "id": "import-timing",
  "kind": "task",
  "category": "audio",
  "next": [
    "native-storyboard"
  ],
  "status": "researching",
  "verification": "reference"
}
---
# Import existing timed lyrics

## Input
$PROJECT, supplied $JSON and original $SRT.

## Command
```sh
mc "$PROJECT" workshop.import_transcript path="$JSON" captions="$SRT"
```

## Output
Existing wording/timing imported into the project.

## Check
Read the receipt. Invalid supplied timing stops for repair; no silent ASR or invented line resynchronization.
