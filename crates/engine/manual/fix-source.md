---
{
  "id": "fix-source",
  "kind": "task",
  "category": "correction",
  "next": [
    "recheck-fix"
  ],
  "status": "researching",
  "verification": "reference"
}
---
# Follow the assigned source task

## Input
$PROJECT and the exact assigned $TASK_ID.

## Command
```sh
mc "$PROJECT" skills.info id="$TASK_ID"
```

## Output
Exact source instructions used for the authorized fix.

## Check
Record changed source and new encoded version; do not patch a review screenshot.
