import bpy
import json
import requests
import bmesh
import math
import time

MAKIT_SERVER_URL = "http://localhost:8085/geometry"

def get_geometry_data():
    """Extract geometry from selected objects"""
    data = {"layout": {"layout_type": "blender_sync"}, "geometry": []}
`   `````    
    # Use selected objects ONLY
    objects = bpy.context.selected_objects
        
    for obj in objects:
        if obj.type != 'MESH':
            continue
            
        # Create a temporary mesh with modifiers applied
        depsgraph = bpy.context.evaluated_depsgraph_get()
        obj_eval = obj.evaluated_get(depsgraph)
        mesh = obj_eval.to_mesh()
        
        # Transform to world coordinates
        matrix = obj.matrix_world
        ````````````````
        # We'll treat each face as a potential polygon to render
        # For simple visualization in TUI, we want 3D coordinates
        
        for poly in mesh.polygons:
            # Get vertices for this face
            points = []
            for loop_index in poly.loop_indices:
                vertex_index = mesh.loops[loop_index].vertex_index
                v = mesh.vertices[vertex_index].co
                # Apply world transform
                world_v = matrix @ v
                # Send as [x, y, z] array for Go compatibility
                points.append([world_v.x, world_v.y, world_v.z])
            
            # Simple heuristic for type
            # Vertical-ish faces are walls, horizontal are floors?
            # For now, let's just make everything "wall_face" unless it's very small
            face_type = "wall_face"
            if poly.area < 0.5: 
                face_type = "window_face" # Just for testing variety
                
            data["geometry"].append({
                "type": face_type,
                "points": points,
                "normal": {"x": poly.normal.x, "y": poly.normal.y, "z": poly.normal.z}
            })
            
        obj_eval.to_mesh_clear()
        
    return data

def push_to_makit():
    try:
        data = get_geometry_data()
        
        # Format explicitly for what our Makit TUI expects
        # The TUI expects a specific structure for 'isometric' viewing usually,
        # but our new endpoint will likely accept a more generic structure 
        # that we adapt on the Go side.
        
        payload = {
            "source": "blender",
            "timestamp": time.time(),
            "data": data
        }
        
        print(f"Sending {len(data['geometry'])} faces to Makit...")
        response = requests.post(MAKIT_SERVER_URL, json=payload, timeout=5.0)
        print(f"Response: {response.status_code}")
        
    except Exception as e:
        print(f"Error connecting to Makit: {e}")

class MakitDumpsOperator(bpy.types.Operator):
    """Send Geometry to Makit"""
    bl_idname = "makit.send_geometry"
    bl_label = "Send to Makit"

    def execute(self, context):
        push_to_makit()
        return {'FINISHED'}

class MakitPanel(bpy.types.Panel):
    """Makit Tools Panel"""
    bl_label = "Makit"
    bl_idname = "VIEW3D_PT_makit"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'Makit'

    def draw(self, context):
        layout = self.layout
        layout.operator("makit.send_geometry")

# Registration
def register():
    bpy.utils.register_class(MakitDumpsOperator)
    bpy.utils.register_class(MakitPanel)

def unregister():
    bpy.utils.unregister_class(MakitDumpsOperator)
    bpy.utils.unregister_class(MakitPanel)

if __name__ == "__main__":
    register()
    # Optional: Auto-run once if executed as script
    # push_to_makit()
