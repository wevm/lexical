#!/usr/bin/env python3
"""Writes Resources/AppIcon.icon, the playground's app icon, from the Lexical
logo mark.

The mark is the left part of the site's wordmark (packages/lexical-website/
static/img/logo.svg, the same file as packages/lexical-playground/public/
logo.svg): a frame with text bars and a text cursor. Its paths are copied as
they are, in three layers of an Icon Composer document (Xcode 26's layered
`.icon`, compiled for iOS and macOS), so the system derives the clear and
tinted appearances:

- Frame: the frame and the I-beam;
- Text: the text bars in the frame's color;
- Accent: the blue text bars.

Each layer's fill is set per appearance from the two logos: logo.svg's dark
mark on a light background, logo-dark.svg's white mark on #1c1d1f (the
favicon's and apple-touch-icon's background is the same dark gray), with the
bars' blue in both. Run it from anywhere after a change to the logo:

    python3 swift/apps/lexical-playground-swift/scripts/make-app-icon.py
"""

import json
import re
import shutil
from pathlib import Path

app = Path(__file__).resolve().parent.parent
repo = app.parent.parent.parent
img = repo / "packages/lexical-website/static/img"
out = app / "Resources/AppIcon.icon"

# The mark's box in the wordmark's coordinates: x 0 to 118, y 22 to 134
# (the I-beam's ends). The word starts at x 148.
MARK_RIGHT = 118
WORD_LEFT = 148
# Apple's icon grid on the 1024-point canvas: the mark 590 points wide, about
# 58% of the canvas, centered on the mark's box.
CANVAS = 1024
SCALE = 5
CENTER = (MARK_RIGHT / 2, (22 + 134) / 2)
BACKGROUND = {"light": "#ffffff", "dark": "#1c1d1f"}


def paths(svg):
    """The (fill, d) of each path of the mark, split into subpaths."""
    result = []
    for tag in re.findall(r"<path\b[^>]*>", svg):
        fill = re.search(r'fill="([^"]+)"', tag).group(1).lower()
        d = re.search(r'\bd="([^"]+)"', tag).group(1)
        for sub in re.findall(r"M[^M]*", d):
            # A subpath's first point is where it starts: in the mark for the
            # mark's shapes, in the word for the word's.
            x = float(re.match(r"M\s*(-?[\d.]+)", sub).group(1))
            if x < WORD_LEFT:
                result.append((fill, sub))
    return result


def layer(subpaths):
    dx = CANVAS / 2 - CENTER[0] * SCALE
    dy = CANVAS / 2 - CENTER[1] * SCALE
    d = "".join(subpaths)
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{CANVAS}" height="{CANVAS}" '
        f'viewBox="0 0 {CANVAS} {CANVAS}"><path transform="translate({dx:g} {dy:g}) '
        f'scale({SCALE})" fill="#000000" d="{d}"/></svg>\n'
    )


def color(hex_):
    """`#rgb` or `#rrggbb` as Icon Composer's `srgb:r,g,b,a`."""
    digits = hex_.lstrip("#")
    if len(digits) == 3:
        digits = "".join(c * 2 for c in digits)
    r, g, b = (int(digits[i : i + 2], 16) / 255 for i in (0, 2, 4))
    return f"srgb:{r:.5f},{g:.5f},{b:.5f},1.00000"


def fills(light, dark, tinted=None):
    result = [
        {"value": {"solid": color(light)}},
        {"appearance": "dark", "value": {"solid": color(dark)}},
    ]
    if tinted:
        result.append({"appearance": "tinted", "value": {"solid": color(tinted)}})
    return result


light = paths((img / "logo.svg").read_text())
dark = paths((img / "logo-dark.svg").read_text())
assert [d for _, d in light] == [d for _, d in dark], "the two logos' marks differ"
accent = next(f for f, _ in light if f != "#000")
assert all(f == accent for f, _ in dark if f not in ("#fff", "#ffffff"))
frame_light, frame_dark = "#000000", "#ffffff"

frame, text, blue = [], [], []
for (fill, d) in light:
    if fill == accent:
        blue.append(d)
    # The bars are the 10-point-high rectangles inside the frame.
    elif re.fullmatch(r"M\d+ \d+h\d+v10(H\d+|h-\d+)V\d+Z", d):
        text.append(d)
    else:
        frame.append(d)

if out.exists():
    shutil.rmtree(out)
(out / "Assets").mkdir(parents=True)
layers = [
    ("Accent", blue, fills(accent, accent)),
    # The tinted and clear appearances draw a layer by its lightness, where
    # the light fill's black would vanish: the mark is white there, as in
    # the dark logo.
    ("Text", text, fills(frame_light, frame_dark, frame_dark)),
    ("Frame", frame, fills(frame_light, frame_dark, frame_dark)),
]
for name, subpaths, _ in layers:
    (out / "Assets" / f"{name}.svg").write_text(layer(subpaths))

icon = {
    "fill-specializations": fills(BACKGROUND["light"], BACKGROUND["dark"]),
    "groups": [
        {
            "layers": [
                {
                    "fill-specializations": layer_fills,
                    "glass": False,
                    "image-name": f"{name}.svg",
                    "name": name,
                }
                for name, _, layer_fills in layers
            ],
            "shadow": {"kind": "neutral", "opacity": 0.5},
            "translucency": {"enabled": False, "value": 0.5},
        }
    ],
    "supported-platforms": {"squares": "shared"},
}
(out / "icon.json").write_text(json.dumps(icon, indent=2, sort_keys=True) + "\n")
print(f"Wrote {out.relative_to(repo)}: {', '.join(f'{n} ({len(s)} shapes)' for n, s, _ in layers)}")
