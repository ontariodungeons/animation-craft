import json
import os
import shutil
import subprocess
import urllib.request


def classify_archetype_from_hint(hint_name: str = "", aspect: float = 1.0) -> dict:
    h = os.path.basename(hint_name).lower()
    if any(k in h for k in ("fish", "fin", "shark", "eel", "snake", "whale")):
        return {
            "archetype": "axial_creature",
            "slicingStrategy": "auto_slice_appendages",
            "primaryVerb": "move_traverse",
        }
    if any(k in h for k in ("sun", "star", "flower", "cloud", "ball", "orb")):
        return {
            "archetype": "radial_prop",
            "slicingStrategy": "single_deformable_sheet",
            "primaryVerb": "jump_rise_fall",
        }
    if any(k in h for k in ("boat", "ship", "car", "tractor", "cart", "vessel")):
        return {
            "archetype": "vehicle_hull",
            "slicingStrategy": "auto_slice_appendages",
            "primaryVerb": "turn_bank",
        }
    return {
        "archetype": "humanoid_biped",
        "slicingStrategy": "base_mannequin_plus_costume_skin",
        "primaryVerb": "move_traverse",
    }


def inspect_image_with_photocraft(image_path: str) -> dict:
    if not image_path or not os.path.exists(image_path):
        return {"width": 512, "height": 512, "aspect": 1.0, "exists": False}
    photocraft = shutil.which("photocraft-cli") or "photocraft-cli"
    try:
        out = subprocess.check_output(
            [photocraft, "info", image_path, "--json"],
            stderr=subprocess.DEVNULL,
            timeout=5,
        ).decode("utf-8")
        info = json.loads(out)
        w = int(info.get("width", 512))
        h = int(info.get("height", 512))
        return {"width": w, "height": h, "aspect": round(w / max(1, h), 3), "exists": True}
    except Exception:
        return {"width": 512, "height": 512, "aspect": 1.0, "exists": True}


def run_three_question_intake(image_path: str, answers_override: dict | None = None) -> dict:
    img_meta = inspect_image_with_photocraft(image_path)
    rec = classify_archetype_from_hint(image_path, img_meta["aspect"])

    api_key = os.environ.get("TYPESAFE_API_KEY", "")
    router_source = "deterministic-heuristic"
    if api_key:
        try:
            payload = json.dumps({
                "model": "jev-latest",
                "primitive": "choice",
                "prompt": f"Classify 2D asset '{os.path.basename(image_path)}' (aspect={img_meta['aspect']}) into skeleton archetype:",
                "choices": ["humanoid_biped", "axial_creature", "radial_prop", "vehicle_hull"],
            }).encode("utf-8")
            req = urllib.request.Request(
                "https://api.typesafe.ai/v1/systemone",
                data=payload,
                headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
                method="POST",
            )
            with urllib.request.urlopen(req, timeout=4) as resp:
                data = json.loads(resp.read().decode("utf-8"))
                if data.get("selection") in ("humanoid_biped", "axial_creature", "radial_prop", "vehicle_hull"):
                    rec["archetype"] = data["selection"]
                    router_source = "typesafe-jev-latest"
        except Exception:
            pass

    questions = [
        {
            "id": "q1_archetype",
            "question": "1. How does this object's core skeleton move?",
            "options": [
                {"value": "humanoid_biped", "label": "Humanoid / Farm Character (18-bone walk, jump, knee/elbow/paw + face)"},
                {"value": "axial_creature", "label": "Fish / Swimming Creature (Spine S-wave + oscillating pectoral/tail fins)"},
                {"value": "radial_prop", "label": "Smiling Sun / Prop (Rise & fall arc + pulsing radial rays + face morphs)"},
                {"value": "vehicle_hull", "label": "Boat / Vehicle (Left/Right traverse + banking yaw turn + wave pitch)"},
            ],
        },
        {
            "id": "q2_slicing",
            "question": "2. How should the 2D image be broken into parts or skins?",
            "options": [
                {"value": "base_mannequin_plus_costume_skin", "label": "Base Skeleton + Swappable Costume Skin"},
                {"value": "auto_slice_appendages", "label": "Slice Moving Appendages (Separate fins/wheels/limbs with 3-loop hinges)"},
                {"value": "single_deformable_sheet", "label": "Single Deformable 2.5D Mesh with Internal Bend Loops"},
            ],
        },
        {
            "id": "q3_verb_output",
            "question": "3. Which Universal Action & Track B Output do you want to verify in Blender 5.3 & FilmCraft?",
            "options": [
                {"value": "move_traverse", "label": "Move / Traverse (Walk / Swim / Glide) -> Blender 5.3 -> FilmCraft"},
                {"value": "jump_rise_fall", "label": "Jump / Rise & Fall (Leap / Sunrise-Sunset / Crest) -> Blender 5.3 -> FilmCraft"},
                {"value": "turn_bank", "label": "Turn / Bank (Set Angles 0°, 40°, 90°, 180°) -> Blender 5.3 -> FilmCraft"},
                {"value": "express_talk", "label": "Express / Talk (Blink, Gaze & Mouth Visemes) -> Blender 5.3 -> FilmCraft"},
            ],
        },
    ]

    recommended = {
        "archetype": rec["archetype"],
        "slicingStrategy": rec["slicingStrategy"],
        "primaryVerb": rec["primaryVerb"],
        "cameraAngles": [0, 40, 90, 180],
        "stripCostumeToBase": False,
    }
    if answers_override:
        recommended.update(answers_override)

    return {
        "imagePath": image_path,
        "imageMeta": img_meta,
        "routerSource": router_source,
        "questions": questions,
        "recommended": recommended,
    }
