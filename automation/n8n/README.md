# Long-Form YouTube Creator Agent

This is the creator half of the RoboNuggets tutorial (the n8n graph in the video posted by Anshika Pathak). The ideas half stays in ChatGPT: paste `prompts/ideas-agent.txt`, then copy the table into the Ideas tab.

One workflow serves every project. Each enabled row in the Projects tab is one show. Each hour, that show renders the first Ideas row whose PRODUCTION STATUS is `to create`, uploads it to YouTube as unlisted, and marks the row published.

## Sheet

Create a Google Sheet and import the three CSVs in `sheets/` as tabs named `Projects`, `Ideas`, and `Assets`.

Put the spreadsheet ID (the long id in the sheet URL) in two places:

- The `Read Projects` node, replacing `REPLACE_WITH_CONTROL_SHEET_ID`
- The `sheet_id` cell of each project row. For a single show, that is the same spreadsheet.

On the Assets tab, put a direct MP4 url in `intro video` and a direct MP3 url in `audio list`. The row you want used has `to use` set to `final`. To rotate songs, point `audio list` at `=INDEX(...)` the way the tutorial does.

A second show is a new Projects row (`enabled` = `yes`) whose `sheet_id` points at that show's own copy of the Ideas and Assets tabs. Leave `enabled` as `no` until it should run.

## JSON2Video template

In the JSON2Video dashboard, add a template and paste `json2video/longform-template.json` as the movie layout. Copy the template id into the project row.

The layout expects the variable names the workflow sends: `voiceModel`, `voiceID`, `imageModel`, `audioURL`, `introVideo`, `introText`, and `clips` (14 objects with `voiceText` and `image_prompt`).

The voice id `ODq5zmih8GrVes37Dizd` is the ElevenLabs voice from the tutorial. It only works if that voice is available on your JSON2Video account. The disabled `second-show` row uses Azure `en-US-AndrewNeural` and `flux-schnell`, which is the free test path from the video.

JSON2Video's free plan renders up to 60 seconds. Fourteen scenes of two sentences will be longer than that.

## n8n

Use the lazydocker project `n8n` at `/home/jesse/docker/projects/n8n`. It is a fresh install: Postgres 18, instance MCP turned on, editor at http://localhost:5678. **Long-Form YouTube Creator Agent** is imported and inactive. Do not start a second n8n.

Credentials the nodes ask for:

| Credential | What to enter |
| --- | --- |
| Google Sheets account | Google OAuth |
| OpenAI account | API key, model gpt-4o |
| JSON2Video API | Header Auth. Name `x-api-key`. Value is your key, with no Bearer prefix. |
| YouTube account | OAuth client id and secret |

Use Test Run before turning on Run Every Hour. The workflow is imported inactive.

## What follows the tutorial, and what was corrected

The graph is his three sections: input (sheet to 14 scene prompts), creation (music, intro, JSON2Video, poll), output (sheet, download, unlisted YouTube upload).

Corrected so a run can finish:

- Render status `pending` and `running` poll again. Copies of this workflow check for `not published`, which JSON2Video never returns.
- All 14 scenes are sent. Those copies skip scene 8.
- The prompt's phase list sums to 14 scenes. The spoken version also says "scenes 14 to 17" while asking for 14 objects.
- YouTube privacy comes from the project row and defaults to unlisted.
- Titles and prompts are passed with `JSON.stringify`, so a quote in the title does not break the render body.
