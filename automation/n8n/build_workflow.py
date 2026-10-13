#!/usr/bin/env python3
"""Regenerate longform-youtube-creator.json and the JSON2Video template."""
import json
import uuid
from pathlib import Path

root = Path(__file__).resolve().parent
system = (root / "prompts/creator-system.txt").read_text().strip()


def uid():
    return str(uuid.uuid4())


SHEET_SCHEMA = [
    {"id": "ID", "displayName": "ID", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True},
    {"id": "IDEAS", "displayName": "IDEAS", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True},
    {"id": "CHANNEL_STYLE_PROMPT", "displayName": "CHANNEL_STYLE_PROMPT", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True},
    {"id": "CHARACTER_STYLE_PROMPT", "displayName": "CHARACTER_STYLE_PROMPT", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True},
    {"id": "CAPTIONS", "displayName": "CAPTIONS", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True},
    {"id": "PRODUCTION STATUS", "displayName": "PRODUCTION STATUS", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True},
    {"id": "FINAL OUTPUT", "displayName": "FINAL OUTPUT", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True},
    {"id": "PUBLISHING STATUS", "displayName": "PUBLISHING STATUS", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True},
    {"id": "ERRORS", "displayName": "ERRORS", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True},
    {"id": "row_number", "displayName": "row_number", "required": False, "defaultMatch": False, "display": True, "type": "string", "canBeUsedToMatch": True, "readOnly": True, "removed": True},
]


def doc(value):
    return {"__rl": True, "value": value, "mode": "id"}


def tab(value):
    return {"__rl": True, "value": value, "mode": "name"}


def sheets_update(values):
    return {
        "operation": "update",
        "documentId": doc("={{ $('Read Projects').item.json.sheet_id }}"),
        "sheetName": tab("={{ $('Read Projects').item.json.ideas_sheet }}"),
        "columns": {
            "mappingMode": "defineBelow",
            "value": values,
            "matchingColumns": ["ID"],
            "schema": SHEET_SCHEMA,
            "attemptToConvertTypes": False,
            "convertFieldsToString": False,
        },
        "options": {},
    }


gs_cred = {"googleSheetsOAuth2Api": {"name": "Google Sheets account"}}
or_cred = {"openRouterApi": {"id": "G45iPmM0vyMIKd8J", "name": "n8n Assistant model"}}
hd_cred = {"httpHeaderAuth": {"name": "JSON2Video API"}}
ol_cred = {"ollamaApi": {"id": "ohN478gATV2UfVOV", "name": "Ollama account"}}
yt_cred = {"youTubeOAuth2Api": {"name": "YouTube account"}}

json_body = """={
  "template": {{ JSON.stringify($('Read Projects').item.json.template_id) }},
  "variables": {
    "voiceModel": {{ JSON.stringify($('Read Projects').item.json.voice_model) }},
    "voiceID": {{ JSON.stringify($('Read Projects').item.json.voice_id) }},
    "imageModel": {{ JSON.stringify($('Read Projects').item.json.image_model) }},
    "audioURL": {{ JSON.stringify($json['audio list']) }},
    "introVideo": {{ JSON.stringify($json['intro video']) }},
    "introText": {{ JSON.stringify($('Google Sheets').item.json.IDEAS) }},
    "clips": {{ JSON.stringify($('Scene List').item.json.clips) }}
  }
}"""

scene_code = r"""const body = $input.item.json;
let clips = body.output ?? body.text ?? body.content ?? body;
if (typeof clips === 'string') clips = JSON.parse(clips);
if (clips && !Array.isArray(clips) && Array.isArray(clips.output)) clips = clips.output;
if (!Array.isArray(clips)) {
  throw new Error('Prompts did not return a scene array.');
}
const normalized = clips.map((scene) => ({
  voiceText: String(scene.voiceText || scene.voice_text || '').trim(),
  image_prompt: String(scene.image_prompt || scene.imagePrompt || '').trim(),
}));
if (normalized.length !== 14) {
  throw new Error('Expected 14 scenes, got ' + normalized.length + '.');
}
const blank = normalized.findIndex((scene) => !scene.voiceText || !scene.image_prompt);
if (blank !== -1) {
  throw new Error('Scene ' + blank + ' is missing voiceText or image_prompt.');
}
return { json: { clips: normalized, sceneCount: normalized.length } };"""

status_opts = {"caseSensitive": True, "leftValue": "", "typeValidation": "loose", "version": 2}


def cond(left, right, op="equals", typ="string"):
    return {
        "id": uid(),
        "leftValue": left,
        "rightValue": right,
        "operator": {"type": typ, "operation": op},
    }


def link(dest, kind="main", index=0):
    return {"node": dest, "type": kind, "index": index}


nodes = [
    {
        "parameters": {
            "content": "## Input\nOne row in Projects = one video project.\nThe schedule reads every row with enabled = yes, then takes the first Ideas row marked to create.\n\nSet the Google Sheet ID on Read Projects only. Later Sheets nodes use that project's sheet_id.",
            "height": 280,
            "width": 520,
            "color": 7,
        },
        "type": "n8n-nodes-base.stickyNote",
        "typeVersion": 1,
        "position": [-80, -260],
        "id": uid(),
        "name": "Note Input",
    },
    {
        "parameters": {
            "content": "## Creation\nThe local 3B model rewrites the 14 scenes. The 1B decision model scores them.\nThe run then waits on an approval form. Approve starts the local Piper and ffmpeg render.",
            "height": 220,
            "width": 640,
            "color": 5,
        },
        "type": "n8n-nodes-base.stickyNote",
        "typeVersion": 1,
        "position": [1100, -200],
        "id": uid(),
        "name": "Note Creation",
    },
    {
        "parameters": {
            "content": "## Output\nDone: write the MP4 URL, download the file, upload to YouTube as unlisted, mark the row published.\nError or a 20 minute timeout: set PRODUCTION STATUS to error and store movie.message.",
            "height": 200,
            "width": 560,
            "color": 4,
        },
        "type": "n8n-nodes-base.stickyNote",
        "typeVersion": 1,
        "position": [2360, -180],
        "id": uid(),
        "name": "Note Output",
    },
    {
        "parameters": {"rule": {"interval": [{"field": "hours", "hoursInterval": 1}]}},
        "type": "n8n-nodes-base.scheduleTrigger",
        "typeVersion": 1.2,
        "position": [0, 80],
        "id": uid(),
        "name": "Run Every Hour",
    },
    {
        "parameters": {},
        "type": "n8n-nodes-base.manualTrigger",
        "typeVersion": 1,
        "position": [0, 280],
        "id": uid(),
        "name": "Test Run",
    },
    {
        "parameters": {
            "resource": "sheet",
            "operation": "read",
            "documentId": doc("REPLACE_WITH_CONTROL_SHEET_ID"),
            "sheetName": tab("Projects"),
            "filtersUI": {"values": [{"lookupColumn": "enabled", "lookupValue": "yes"}]},
            "options": {},
        },
        "type": "n8n-nodes-base.googleSheets",
        "typeVersion": 4.5,
        "position": [260, 180],
        "id": uid(),
        "name": "Read Projects",
        "credentials": gs_cred,
    },
    {
        "parameters": {
            "resource": "sheet",
            "operation": "read",
            "documentId": doc("={{ $('Read Projects').item.json.sheet_id }}"),
            "sheetName": tab("={{ $('Read Projects').item.json.ideas_sheet }}"),
            "filtersUI": {"values": [{"lookupColumn": "PRODUCTION STATUS", "lookupValue": "to create"}]},
            "options": {"returnFirstMatch": True},
        },
        "type": "n8n-nodes-base.googleSheets",
        "typeVersion": 4.5,
        "position": [500, 180],
        "id": uid(),
        "name": "Google Sheets",
        "credentials": gs_cred,
    },
    {
        "parameters": {
            "promptType": "define",
            "text": "=Video Title: {{ $json.IDEAS }}\nVideo Description: {{ $json.CAPTIONS }}",
            "hasOutputParser": True,
            "messages": {"messageValues": [{"message": "=" + system}]},
        },
        "type": "@n8n/n8n-nodes-langchain.chainLlm",
        "typeVersion": 1.6,
        "position": [760, 180],
        "id": uid(),
        "name": "Prompts",
    },
    {
        "parameters": {
            "model": "={{ $('Read Projects').item.json.text_provider === 'claude' ? 'anthropic/claude-haiku-4.5' : 'openai/gpt-4.1-mini' }}",
            "options": {},
        },
        "type": "@n8n/n8n-nodes-langchain.lmChatOpenRouter",
        "typeVersion": 1,
        "position": [720, 400],
        "id": uid(),
        "name": "OpenAI or Claude",
        "credentials": or_cred,
    },
    {
        "parameters": {
            "jsonSchemaExample": '[\n  {\n    "voiceText": "The warrior stands still, and the noise of the world drops away.",\n    "image_prompt": "A Spartan hoplite at dawn, bronze and crimson, calm eyes, dust and olive trees, cinematic wide shot."\n  }\n]'
        },
        "type": "@n8n/n8n-nodes-langchain.outputParserStructured",
        "typeVersion": 1.2,
        "position": [920, 400],
        "id": uid(),
        "name": "Structured Output Parser",
    },
    {
        "parameters": {"mode": "runOnceForEachItem", "language": "javaScript", "jsCode": scene_code},
        "type": "n8n-nodes-base.code",
        "typeVersion": 2,
        "position": [1020, 180],
        "id": uid(),
        "name": "Scene List",
    },
    {
        "parameters": {
            "resource": "sheet",
            "operation": "read",
            "documentId": doc("={{ $('Read Projects').item.json.sheet_id }}"),
            "sheetName": tab("={{ $('Read Projects').item.json.assets_sheet }}"),
            "filtersUI": {"values": [{"lookupColumn": "to use", "lookupValue": "final"}]},
            "options": {"returnFirstMatch": True},
        },
        "type": "n8n-nodes-base.googleSheets",
        "typeVersion": 4.5,
        "position": [1240, 180],
        "id": uid(),
        "name": "Get Music",
        "credentials": gs_cred,
    },
    {
        "parameters": {
            "method": "POST",
            "url": "https://api.json2video.com/v2/movies",
            "authentication": "genericCredentialType",
            "genericAuthType": "httpHeaderAuth",
            "sendBody": True,
            "specifyBody": "json",
            "jsonBody": json_body,
            "options": {},
        },
        "type": "n8n-nodes-base.httpRequest",
        "typeVersion": 4.2,
        "position": [1480, 180],
        "id": uid(),
        "name": "Create Movie",
        "credentials": hd_cred,
    },
    {
        "parameters": {"amount": 30, "unit": "seconds"},
        "type": "n8n-nodes-base.wait",
        "typeVersion": 1.1,
        "position": [1700, 180],
        "id": uid(),
        "name": "Wait for Render",
    },
    {
        "parameters": {
            "url": "https://api.json2video.com/v2/movies",
            "authentication": "genericCredentialType",
            "genericAuthType": "httpHeaderAuth",
            "sendQuery": True,
            "queryParameters": {
                "parameters": [{"name": "project", "value": "={{ $('Create Movie').item.json.project }}"}]
            },
            "options": {},
        },
        "type": "n8n-nodes-base.httpRequest",
        "typeVersion": 4.2,
        "position": [1920, 180],
        "id": uid(),
        "name": "Check Movie Status",
        "credentials": hd_cred,
    },
    {
        "parameters": {
            "rules": {
                "values": [
                    {
                        "conditions": {
                            "options": status_opts,
                            "conditions": [cond("={{ $json.movie.status }}", "done")],
                            "combinator": "and",
                        },
                        "renameOutput": True,
                        "outputKey": "done",
                    },
                    {
                        "conditions": {
                            "options": status_opts,
                            "conditions": [cond("={{ $json.movie.status }}", "error")],
                            "combinator": "and",
                        },
                        "renameOutput": True,
                        "outputKey": "error",
                    },
                ]
            },
            "options": {"fallbackOutput": "extra"},
        },
        "type": "n8n-nodes-base.switch",
        "typeVersion": 3.2,
        "position": [2140, 160],
        "id": uid(),
        "name": "Route by Status",
    },
    {
        "parameters": {
            "conditions": {
                "options": {"caseSensitive": True, "leftValue": "", "typeValidation": "loose", "version": 2},
                "conditions": [cond("={{ $runIndex }}", 40, "gt", "number")],
                "combinator": "and",
            },
            "options": {},
        },
        "type": "n8n-nodes-base.if",
        "typeVersion": 2.2,
        "position": [2380, 360],
        "id": uid(),
        "name": "Still Rendering?",
    },
    {
        "parameters": {"amount": 30, "unit": "seconds"},
        "type": "n8n-nodes-base.wait",
        "typeVersion": 1.1,
        "position": [2140, 520],
        "id": uid(),
        "name": "Wait and Recheck",
    },
    {
        "parameters": sheets_update({
            "ID": "={{ $('Google Sheets').item.json.ID }}",
            "PRODUCTION STATUS": "error",
            "ERRORS": "={{ $json.movie.message || 'Render timed out after 40 status checks' }}",
        }),
        "type": "n8n-nodes-base.googleSheets",
        "typeVersion": 4.5,
        "position": [2620, 360],
        "id": uid(),
        "name": "Error log",
        "credentials": gs_cred,
    },
    {
        "parameters": sheets_update({
            "ID": "={{ $('Google Sheets').item.json.ID }}",
            "PRODUCTION STATUS": "created",
            "FINAL OUTPUT": "={{ $('Check Movie Status').item.json.movie.url }}",
        }),
        "type": "n8n-nodes-base.googleSheets",
        "typeVersion": 4.5,
        "position": [2380, 40],
        "id": uid(),
        "name": "Save Video URL",
        "credentials": gs_cred,
    },
    {
        "parameters": {
            "url": "={{ $('Check Movie Status').item.json.movie.url }}",
            "options": {"response": {"response": {"responseFormat": "file", "outputPropertyName": "data"}}},
        },
        "type": "n8n-nodes-base.httpRequest",
        "typeVersion": 4.2,
        "position": [2620, 40],
        "id": uid(),
        "name": "Download Video",
    },
    {
        "parameters": {
            "resource": "video",
            "operation": "upload",
            "title": "={{ $('Google Sheets').item.json.IDEAS }}",
            "regionCode": "={{ $('Read Projects').item.json.youtube_region }}",
            "categoryId": "={{ $('Read Projects').item.json.youtube_category }}",
            "options": {
                "description": "={{ $('Google Sheets').item.json.CAPTIONS }}",
                "privacyStatus": "={{ $('Read Projects').item.json.youtube_privacy }}",
            },
        },
        "type": "n8n-nodes-base.youTube",
        "typeVersion": 1,
        "position": [2860, 40],
        "id": uid(),
        "name": "YouTube",
        "credentials": yt_cred,
    },
    {
        "parameters": sheets_update({
            "ID": "={{ $('Google Sheets').item.json.ID }}",
            "PUBLISHING STATUS": "published",
        }),
        "type": "n8n-nodes-base.googleSheets",
        "typeVersion": 4.5,
        "position": [3100, 40],
        "id": uid(),
        "name": "Done",
        "credentials": gs_cred,
    },
]

connections = {
    "Run Every Hour": {"main": [[link("Read Projects")]]},
    "Test Run": {"main": [[link("Read Projects")]]},
    "Read Projects": {"main": [[link("Google Sheets")]]},
    "Google Sheets": {"main": [[link("Prompts")]]},
    "OpenAI Chat Model": {"ai_languageModel": [[link("Prompts", kind="ai_languageModel")]]},
    "Structured Output Parser": {"ai_outputParser": [[link("Prompts", kind="ai_outputParser")]]},
    "Prompts": {"main": [[link("Scene List")]]},
    "Scene List": {"main": [[link("Get Music")]]},
    "Get Music": {"main": [[link("Create Movie")]]},
    "Create Movie": {"main": [[link("Wait for Render")]]},
    "Wait for Render": {"main": [[link("Check Movie Status")]]},
    "Check Movie Status": {"main": [[link("Route by Status")]]},
    "Route by Status": {"main": [[link("Save Video URL")], [link("Error log")], [link("Still Rendering?")]]},
    "Still Rendering?": {"main": [[link("Error log")], [link("Wait and Recheck")]]},
    "Wait and Recheck": {"main": [[link("Check Movie Status")]]},
    "Save Video URL": {"main": [[link("Download Video")]]},
    "Download Video": {"main": [[link("YouTube")]]},
    "YouTube": {"main": [[link("Done")]]},
}

names = {n["name"] for n in nodes}
for src, kinds in connections.items():
    if src not in names:
        raise SystemExit(f"missing source {src}")
    for groups in kinds.values():
        for group in groups:
            for edge in group:
                if edge["node"] not in names:
                    raise SystemExit(f"missing dest {edge['node']}")

workflow = {
    "id": "omhasLongformYt1",
    "name": "Long-Form YouTube Creator Agent",
    "nodes": nodes,
    "connections": connections,
    "pinData": {},
    "active": False,
    "settings": {"executionOrder": "v1"},
    "meta": {"templateCredsSetupCompleted": False},
}
out = root / "longform-youtube-creator.json"
out.write_text(json.dumps(workflow, indent=2) + "\n")

scenes = [
    {
        "comment": "intro",
        "duration": 4,
        "elements": [
            {"type": "video", "src": "{{introVideo}}", "resize": "cover"},
            {"type": "text", "text": "{{introText}}", "style": "001", "duration": 4},
        ],
    }
]
for i in range(14):
    scenes.append(
        {
            "comment": f"scene {i}",
            "elements": [
                {"type": "image", "model": "{{imageModel}}", "prompt": "{{clips.%d.image_prompt}}" % i},
                {"type": "voice", "model": "{{voiceModel}}", "voice": "{{voiceID}}", "text": "{{clips.%d.voiceText}}" % i},
                {"type": "text", "text": "{{clips.%d.voiceText}}" % i, "style": "002"},
            ],
        }
    )
template = {
    "resolution": "full-hd",
    "quality": "high",
    "comment": "Long-form creator. Variable names match the n8n Create Movie body.",
    "elements": [{"type": "audio", "src": "{{audioURL}}", "volume": 0.15, "duration": -1}],
    "scenes": scenes,
}
(root / "json2video/longform-template.json").write_text(json.dumps(template, indent=2) + "\n")
print(f"wrote {out.name} ({len(nodes)} nodes) and template ({len(scenes)} scenes)")
