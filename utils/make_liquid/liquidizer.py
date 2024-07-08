import os
import json
import shutil
import copy

#base_name = input("Name of block, e.g. 'water': ")
#material_name = input("Material name, e.g. 'water': ")
#friction = input("Friction, 'x,y,z': ").split(',')
#light_attenuation = input("Light attenuation (integer):")
#flowing_texture_name = input("Flowing texture name: ")
#still_texture_name = input("Still texture name: ")
base_name = "water"
material_name = "water"
friction = [0.8, 0.5, 0.8]
flowing_texture_name = "flowing_water.png"
still_texture_name = "still_water.png"
fog = {
    "color": {
        "Rgba": {
            "red": 0,
            "green": 0,
            "blue": 1,
            "alpha": 1
        }
    },
    "distance": 50
}

def make_block(name, top_quad, top_texture, rotate_texture, is_rotatable, cull_top=False, light_attenuation=0):
    quads = []
    
    # top
    quads.append({
        "vertices": copy.deepcopy(top_quad),
        "texture": top_texture,
        "rotate_texture": rotate_texture
    })

    if cull_top:
        quads[0]["cull_face"] = "top"

    # bottom
    quads.append({
        "vertices": [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
        ],
        "texture": still_texture_name,
        "cull_face": "bottom"
    })

    #right
    quads.append({
        "vertices": [
            [1.0, top_quad[3][1], 1.0],
            [1.0, 0.0, 1.0],
            [1.0, top_quad[2][1], 0.0],
            [1.0, 0.0, 0.0]
        ],
        "texture": flowing_texture_name,
        "cull_face": "right"
    })

    #left
    quads.append({
        "vertices": [
            [0.0, top_quad[0][1], 0.0],
            [0.0, 0.0, 0.0],
            [0.0, top_quad[1][1], 1.0],
            [0.0, 0.0, 1.0]
        ],
        "texture": flowing_texture_name,
        "cull_face": "left"
    })

    #front
    quads.append({
        "vertices": [
            [0.0, top_quad[1][1], 1.0],
            [0.0, 0.0, 1.0],
            [1.0, top_quad[3][1], 1.0],
            [1.0, 0.0, 1.0]
        ],
        "texture": flowing_texture_name,
        "cull_face": "front"
    })

    #back
    quads.append({
        "vertices": [
            [1.0, top_quad[2][1], 0.0],
            [1.0, 0.0, 0.0],
            [0.0, top_quad[0][1], 0.0],
            [0.0, 0.0, 0.0]
        ],
        "texture": flowing_texture_name,
        "cull_face": "back"
    })

    return {
        "type": "cube",
        "name": name,
        "material": material_name,
        "friction": {
            "drag": friction,
        },
        "light_attenuation": light_attenuation,
        "fog": fog,
        "is_rotatable": is_rotatable,
        "quads": quads
    }



def make_blocks(name, decrement, top_texture, rotate_texture, count, is_rotatable=True):
    blocks = []
    
    # 0  2
    # | /|
    # |/ |
    # 1  3
    # this is the indexing of the decrement list
    top_quad = [
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
    ]

    for i in range(0, 4):
        top_quad[i][1] = round(top_quad[i][1] - decrement[i] * 0.1, 1)

    for block_suffix in reversed(range(1, count + 1)):
        for i in range(0, 4):
            top_quad[i][1] = round(top_quad[i][1] - 0.1, 1)

        block = make_block(name + "_" + str(block_suffix), top_quad, top_texture, rotate_texture, is_rotatable)
        blocks.append(block)

    return blocks

blocks = []

blocks.append(make_block(
    name="subsurface_" + base_name,
    top_quad=[
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0]
    ],
    top_texture=still_texture_name,
    rotate_texture=False,
    is_rotatable=False,
    cull_top=True,
    light_attenuation=1,
))

blocks.append(make_block(
    name="surface_" + base_name,
    top_quad=[
        [0.0, 0.9, 0.0],
        [0.0, 0.9, 1.0],
        [1.0, 0.9, 0.0],
        [1.0, 0.9, 1.0]
    ],
    top_texture=still_texture_name,
    rotate_texture=False,
    is_rotatable=False,
    cull_top=False
))

blocks.append(make_block(
    name="still_" + base_name + "_10",
    top_quad=[
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0]
    ],
    top_texture=still_texture_name,
    rotate_texture=False,
    is_rotatable=False,
    cull_top=True,
    light_attenuation=1,
))

blocks.extend(make_blocks(
    "still_" + base_name,
    [0,0,0,0],
    still_texture_name,
    False,
    9,
    False
))

blocks.extend(make_blocks(
    "tilted_" + base_name,
    [1,0,0,1],
    still_texture_name,
    False,
    8
))

blocks.extend(make_blocks(
    "straight_" + base_name,
    [0,1,0,1],
    flowing_texture_name,
    False,
    8,
))

blocks.extend(make_blocks(
    "diagonal_" + base_name,
    [1,2,0,1],
    flowing_texture_name,
    True,
    7,
))

blocks.extend(make_blocks(
    "diagonal_" + base_name + "_corner_up",
    [1,1,0,1],
    flowing_texture_name,
    True,
    8
))

blocks.extend(make_blocks(
    "diagonal_" + base_name + "_corner_down",
    [0,1,0,0],
    flowing_texture_name,
    True,
    8
))

shutil.rmtree(base_name, ignore_errors=True)
os.mkdir(base_name)

for block in blocks:
    path = base_name + "/" + block["name"] + ".json"
    with open(path, "w") as file:
        json.dump(block, file, indent=4)
