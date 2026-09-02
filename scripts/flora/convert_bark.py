"""Re-encode a Poly Haven bark set (JPG) to the 8-bit PNGs the engine embeds.

    blender --background --python scripts/flora/convert_bark.py -- assets/flora/bark/<id>
"""
import os
import sys

import bpy

folder = os.path.abspath(sys.argv[sys.argv.index("--") + 1])
for name, colorspace in (("diff_1k", "sRGB"), ("nor_gl_1k", "Non-Color")):
    image = bpy.data.images.load(os.path.join(folder, f"{name}.jpg"), check_existing=False)
    image.colorspace_settings.name = colorspace
    # Force the pixel buffer in (a background load is lazy until the pixels are touched).
    _ = image.pixels[0]
    image.filepath_raw = os.path.join(folder, f"{name}.png")
    image.file_format = "PNG"
    image.save()
    print("wrote", image.filepath_raw, image.size[:], image.depth, flush=True)
