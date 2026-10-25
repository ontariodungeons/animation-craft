---
{
  "id": "resolve-project",
  "kind": "task",
  "category": "audio",
  "next": [
    "choose-audio"
  ],
  "status": "researching",
  "verification": "reference"
}
---
# Resolve the requested track

## Input
$TRACK request; existing project history.

## Command
```sh
mc exec project.catalog
```

## Output
Canonical $PROJECT path, or a precise missing-track question.

## Check
Catalog is newest ten projects, not a numbered-track index. Ask for an absent track’s entry/audio path.
