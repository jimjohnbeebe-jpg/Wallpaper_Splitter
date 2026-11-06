"""Packs the rendered icons/ico/<size>.png files into icons/icon.ico (16-256 px), for tools/store-art.cjs output."""
import sys
from pathlib import Path

from PIL import Image

out = Path(sys.argv[1])
sizes = [16, 24, 32, 48, 64, 256]
frames = [Image.open(out / "icons" / "ico" / f"{s}.png").convert("RGBA") for s in sizes]
frames[-1].save(out / "icons" / "icon.ico", sizes=[(s, s) for s in sizes], append_images=frames[:-1])
print("wrote", out / "icons" / "icon.ico")
