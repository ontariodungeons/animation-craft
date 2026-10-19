import type { ArchetypeId, UniversalVerb } from "../types/schema";
import { buildArchetypeSkeleton } from "../archetypes/index";
import type { CutoutRigExportPlan } from "./videoCodeExporter";

export interface Blender53ExportOptions {
  glbPath?: string;
  blendOutPath?: string;
  rigPlan?: CutoutRigExportPlan;
}

export function generateBlender53ArchetypeScript(
  archetype: ArchetypeId,
  verb: UniversalVerb,
  renderOutPath: string,
  options: Blender53ExportOptions = {}
): string {
  const skel = buildArchetypeSkeleton(archetype);
  const bonesJson = JSON.stringify(skel.bones);
  const glbJson = JSON.stringify(options.glbPath ?? null);
  const blendJson = JSON.stringify(options.blendOutPath ?? null);
  const rigPlanJson = JSON.stringify(options.rigPlan ?? null);

  return `import bpy
import math
import json
import os

ARCHETYPE = ${JSON.stringify(archetype)}
VERB = ${JSON.stringify(verb)}
RENDER_OUT = ${JSON.stringify(renderOutPath)}
GLB_PATH = ${glbJson}
BLEND_OUT = ${blendJson}
RIG_PLAN = json.loads(${JSON.stringify(rigPlanJson)})
BONES_SPEC = json.loads(${JSON.stringify(bonesJson)})

scene = bpy.context.scene
scene.render.film_transparent = True

if GLB_PATH and os.path.isfile(GLB_PATH):
    bpy.ops.import_scene.gltf(filepath=GLB_PATH)

def build_modular_armature():
    bpy.ops.object.armature_add(enter_editmode=True, location=(0, 0, 0))
    arm_obj = bpy.context.active_object
    arm_obj.name = f"OMHAS_{ARCHETYPE}_Armature"
    arm_obj.show_in_front = True
    arm = arm_obj.data
    arm.edit_bones.remove(arm.edit_bones[0])

    created = {}
    for b in BONES_SPEC:
        eb = arm.edit_bones.new(b["name"])
        hx, hy, hz = b["head"]
        tx, ty, tz = b["tail"]
        eb.head = (hx, -hz, hy)
        eb.tail = (tx, -tz, ty)
        if b["parent"] and b["parent"] in created:
            eb.parent = created[b["parent"]]
        created[b["name"]] = eb

    bpy.ops.object.mode_set(mode='OBJECT')
    bpy.context.view_layer.update()

    for obj in list(bpy.data.objects):
        if obj.type in ('MESH', 'EMPTY') and obj.userData_bone if hasattr(obj, 'userData_bone') else False:
            bone_name = obj.userData_bone
            if bone_name in arm_obj.pose.bones:
                mw = obj.matrix_world.copy()
                obj.parent = None
                obj.matrix_world = mw
                con = obj.constraints.new('CHILD_OF')
                con.target = arm_obj
                con.subtarget = bone_name
                bone_world_mat = arm_obj.matrix_world @ arm_obj.pose.bones[bone_name].matrix
                con.inverse_matrix = bone_world_mat.inverted()
    return arm_obj

arm_obj = build_modular_armature()
if BLEND_OUT:
    os.makedirs(os.path.dirname(os.path.abspath(BLEND_OUT)), exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=os.path.abspath(BLEND_OUT))
print("Built Blender 5.3 Armature:", arm_obj.name, "for verb:", VERB)
`;
}

