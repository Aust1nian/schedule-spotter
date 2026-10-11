"""Draws the app icon source (1024 px): a week grid with one red cell.
Run: uv run --with pillow app/icon-src/make_icon.py app/icon-src/icon.png
Then: (cd app && npx tauri icon icon-src/icon.png -o src-tauri/icons)"""
import sys
from PIL import Image, ImageDraw

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rounded_rectangle((64, 64, S - 64, S - 64), radius=180, fill=(23, 54, 93, 255))
d.rounded_rectangle((176, 200, S - 176, 300), radius=28, fill=(255, 255, 255, 255))
cols, rows = 5, 3
x0, y0, x1, y1 = 176, 340, S - 176, S - 176
gap = 20
cw = (x1 - x0 - gap * (cols - 1)) / cols
ch = (y1 - y0 - gap * (rows - 1)) / rows
for r in range(rows):
    for c in range(cols):
        fill = (255, 255, 255, 235)
        if (r, c) == (1, 2):
            fill = (214, 59, 47, 255)
        elif (r, c) == (2, 4):
            fill = (240, 176, 48, 255)
        x = x0 + c * (cw + gap)
        y = y0 + r * (ch + gap)
        d.rounded_rectangle((x, y, x + cw, y + ch), radius=16, fill=fill)
img.save(sys.argv[1])
