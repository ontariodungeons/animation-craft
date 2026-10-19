import type { ArchetypeId, IntakeAnswers, IntakeQuestion, SlicingStrategy, UniversalVerb } from "../types/schema";

export function classifyArchetypeFromHint(hintName = "", aspect = 1.0): {
  archetype: ArchetypeId;
  slicingStrategy: SlicingStrategy;
  primaryVerb: UniversalVerb;
} {
  const h = hintName.toLowerCase();
  if (h.includes("fish") || h.includes("fin") || h.includes("shark") || h.includes("eel") || h.includes("snake")) {
    return {
      archetype: "axial_creature",
      slicingStrategy: "auto_slice_appendages",
      primaryVerb: "move_traverse"
    };
  }
  if (h.includes("sun") || h.includes("star") || h.includes("flower") || h.includes("cloud") || h.includes("ball")) {
    return {
      archetype: "radial_prop",
      slicingStrategy: "single_deformable_sheet",
      primaryVerb: "jump_rise_fall"
    };
  }
  if (h.includes("boat") || h.includes("ship") || h.includes("car") || h.includes("tractor") || h.includes("cart")) {
    return {
      archetype: "vehicle_hull",
      slicingStrategy: "auto_slice_appendages",
      primaryVerb: "turn_bank"
    };
  }
  if (aspect > 1.75 && !h.includes("turn") && !h.includes("pose") && !h.includes("slide")) {
    return {
      archetype: "axial_creature",
      slicingStrategy: "auto_slice_appendages",
      primaryVerb: "move_traverse"
    };
  }
  return {
    archetype: "humanoid_biped",
    slicingStrategy: "base_mannequin_plus_costume_skin",
    primaryVerb: "move_traverse"
  };
}

export function buildIntakeQuestions(hintName = "", aspect = 1.0): {
  questions: IntakeQuestion[];
  recommended: IntakeAnswers;
} {
  const rec = classifyArchetypeFromHint(hintName, aspect);
  const questions: IntakeQuestion[] = [
    {
      id: "q1_archetype",
      question: "1. How does this object's core skeleton move?",
      options: [
        { value: "humanoid_biped", label: "Humanoid / Farm Character (18-bone walk, jump, knee/elbow/paw + face)" },
        { value: "axial_creature", label: "Fish / Swimming Creature (Spine S-wave + oscillating pectoral/tail fins)" },
        { value: "radial_prop", label: "Smiling Sun / Prop (Rise & fall arc + pulsing radial rays + face morphs)" },
        { value: "vehicle_hull", label: "Boat / Vehicle (Left/Right traverse + banking yaw turn + wave pitch)" }
      ]
    },
    {
      id: "q2_slicing",
      question: "2. How should the 2D image be broken into parts or skins?",
      options: [
        { value: "base_mannequin_plus_costume_skin", label: "Base Skeleton + Swappable Costume Skin (Video game skin workflow)" },
        { value: "auto_slice_appendages", label: "Slice Moving Appendages (Separate fins/wheels/limbs with 3-loop hinges)" },
        { value: "single_deformable_sheet", label: "Single Deformable 2.5D Mesh with Internal Bend Loops" }
      ]
    },
    {
      id: "q3_verb_output",
      question: "3. Which Universal Action & Track B Output do you want to verify in Blender 5.3 & FilmCraft?",
      options: [
        { value: "move_traverse", label: "Move / Traverse (Walk / Swim / Glide) -> Blender 5.3 -> FilmCraft" },
        { value: "jump_rise_fall", label: "Jump / Rise & Fall (Leap / Sunrise-Sunset / Crest) -> Blender 5.3 -> FilmCraft" },
        { value: "turn_bank", label: "Turn / Bank (Set Angles 0°, 40°, 90°, 180°) -> Blender 5.3 -> FilmCraft" },
        { value: "express_talk", label: "Express / Talk (Blink, Gaze & Mouth Visemes) -> Blender 5.3 -> FilmCraft" }
      ]
    }
  ];

  return {
    questions,
    recommended: {
      archetype: rec.archetype,
      slicingStrategy: rec.slicingStrategy,
      primaryVerb: rec.primaryVerb,
      cameraAngles: [0, 40, 90, 180],
      stripCostumeToBase: false
    }
  };
}

export async function runThreeQuestionIntake(
  hintName = "",
  overrides: Partial<IntakeAnswers> = {},
  typeSafeApiKey = ""
): Promise<{ questions: IntakeQuestion[]; answers: IntakeAnswers; routerSource: string }> {
  const base = buildIntakeQuestions(hintName);
  let routerSource = "deterministic-heuristic";

  if (typeSafeApiKey && typeof fetch === "function") {
    try {
      const resp = await fetch("https://api.typesafe.ai/v1/systemone", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${typeSafeApiKey}`,
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          model: "jev-latest",
          primitive: "choice",
          prompt: `Classify uploaded 2D asset "${hintName}" into the best animation skeleton archetype:`,
          choices: ["humanoid_biped", "axial_creature", "radial_prop", "vehicle_hull"]
        })
      });
      if (resp.ok) {
        const data = await resp.json();
        if (data?.selection) {
          base.recommended.archetype = data.selection as ArchetypeId;
          routerSource = "typesafe-jev-latest";
        }
      }
    } catch {
      // Fallback cleanly to deterministic classification
    }
  }

  return {
    questions: base.questions,
    answers: { ...base.recommended, ...overrides },
    routerSource
  };
}
