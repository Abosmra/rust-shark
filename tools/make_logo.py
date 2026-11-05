"""Generates assets/logo.svg: the notched shark fin built from riveted, rusted iron plates.

Run:  python tools/make_logo.py
"""
import math
import pathlib
import random

random.seed(7)
W = H = 512
BASE_Y = 440

# Fin silhouette: a sweeping fin with a notched trailing edge (Attack Shark homage),
# traced on a 256 grid, then scaled into the 512 tile.
PTS256 = [
    (9, 246), (19, 210), (31, 180), (43, 156), (58, 132), (72, 114), (88, 96), (106, 78),
    (121, 66), (138, 54), (158, 42), (182, 30), (213, 18), (236, 10), (248, 9),
    (241, 30), (235, 48), (232, 56), (226, 60), (204, 72), (212, 79), (218, 85),
    (198, 91), (198, 97), (212, 104), (213, 114), (212, 184), (214, 188), (199, 193),
    (211, 199), (219, 210), (227, 228), (233, 246),
    (162, 246), (156, 232), (150, 206), (146, 180), (144, 160), (144, 119),
    (135, 119), (127, 125), (120, 132), (109, 144), (100, 156), (92, 168), (85, 180),
    (79, 192), (79, 198), (88, 204), (81, 210), (75, 216), (68, 228), (67, 246),
]
S, OX, OY = 1.6, 50, 44
PTS = [(OX + x * S, OY + y * S) for x, y in PTS256]
fin_d = "M" + " L".join(f"{x:.1f},{y:.1f}" for x, y in PTS) + " Z"


def inside(x, y):
    """Even-odd point-in-polygon."""
    c = False
    for (x1, y1), (x2, y2) in zip(PTS, PTS[1:] + PTS[:1]):
        if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
            c = not c
    return c


def solid(x, y, r=11):
    return all(inside(x + dx, y + dy) for dx, dy in ((0, 0), (r, 0), (-r, 0), (0, r), (0, -r)))


# Horizontal plate seams (slightly sagging) and staggered vertical seams, clipped by the fin.
SEAMS_Y = [330, 250, 172, 104]
rows = [440] + SEAMS_Y + [40]
seam_paths = [f"M0,{y} Q{W / 2},{y + 7} {W},{y}" for y in SEAMS_Y]
rivets = []
for y in SEAMS_Y:
    for x in range(20, W, 34):
        sag = 7 * (1 - (2 * x / W - 1) ** 2)
        if solid(x, y + sag + 11, 9):
            rivets.append((x, y + sag + 11))

verticals = []
for i in range(len(rows) - 1):
    y_bot, y_top = rows[i], rows[i + 1]
    for x in ((150, 400) if i % 2 == 0 else (255, 445)):
        verticals.append(f"M{x},{y_bot + 3} L{x + 6},{y_top + 3}")
        for k in range(1, 4):
            y = y_bot + (y_top - y_bot) * k / 4
            if y < BASE_Y and solid(x + 13, y, 9):
                rivets.append((x + 13 + 6 * k / 4, y))

# Plate tints: each band a slightly different rust
band_fills = ["#7d3a17", "#8e4419", "#9a4d1d", "#874019", "#a3561f"]
plates = []
for i in range(len(rows) - 1):
    y_bot, y_top = rows[i], rows[i + 1]
    plates.append(f'<rect x="0" y="{y_top}" width="{W}" height="{y_bot - y_top + 1}" fill="{band_fills[i % len(band_fills)]}"/>')
# Rust streaks running down from some rivets
streaks = []
for (x, y) in random.sample(rivets, k=min(14, len(rivets))):
    length = random.randint(18, 46)
    w = random.uniform(2.0, 3.6)
    streaks.append(
        f'<path d="M{x - w / 2:.1f},{y + 3:.1f} q{w / 2:.1f},{length * 0.6:.1f} {w * 0.2:.1f},{length:.1f} '
        f'q{w * 0.3:.1f},{-length * 0.5:.1f} {w * 0.8:.1f},{-length:.1f}z" fill="url(#streak)"/>'
    )

rivet_svg = "\n".join(
    f'<circle cx="{x:.1f}" cy="{y:.1f}" r="6.2" fill="url(#rivet)"/>'
    f'<circle cx="{x:.1f}" cy="{y + 1.4:.1f}" r="6.2" fill="none" stroke="#2a1206" stroke-opacity=".55" stroke-width="1.2"/>'
    for (x, y) in rivets
)

svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}">
  <defs>
    <clipPath id="fin"><path d="{fin_d}"/></clipPath>

    <!-- fine pitted rust grain -->
    <filter id="grain" x="0" y="0" width="100%" height="100%">
      <feTurbulence type="fractalNoise" baseFrequency="0.85" numOctaves="3" seed="3" result="n"/>
      <feColorMatrix in="n" type="matrix"
        values="0 0 0 0 0.20  0 0 0 0 0.07  0 0 0 0 0.02  0 0 0 -1.6 1.15"/>
    </filter>
    <!-- large blotches of bright orange rust and dark corrosion -->
    <filter id="blotch" x="0" y="0" width="100%" height="100%">
      <feTurbulence type="fractalNoise" baseFrequency="0.018" numOctaves="4" seed="11" result="n"/>
      <feColorMatrix in="n" type="matrix"
        values="0 0 0 0 0.98  0 0 0 0 0.52  0 0 0 0 0.16  0 0 0 5.0 -2.45"/>
    </filter>
    <filter id="corrode" x="0" y="0" width="100%" height="100%">
      <feTurbulence type="fractalNoise" baseFrequency="0.035" numOctaves="3" seed="29" result="n"/>
      <feColorMatrix in="n" type="matrix"
        values="0 0 0 0 0.10  0 0 0 0 0.04  0 0 0 0 0.02  0 0 0 5 -2.7"/>
    </filter>

    <radialGradient id="glow">
      <stop offset="0" stop-color="#ff6a2b" stop-opacity=".22"/>
      <stop offset=".55" stop-color="#ff6a2b" stop-opacity=".07"/>
      <stop offset="1" stop-color="#ff6a2b" stop-opacity="0"/>
    </radialGradient>
    <filter id="soft"><feGaussianBlur stdDeviation="7"/></filter>
    <radialGradient id="rivet" cx="35%" cy="32%" r="70%">
      <stop offset="0" stop-color="#f2b07a"/>
      <stop offset=".35" stop-color="#b8612a"/>
      <stop offset="1" stop-color="#4a1d08"/>
    </radialGradient>
    <linearGradient id="streak" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#5a2208" stop-opacity=".85"/>
      <stop offset="1" stop-color="#5a2208" stop-opacity="0"/>
    </linearGradient>
    <linearGradient id="shade" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0" stop-color="#fff" stop-opacity=".16"/>
      <stop offset=".45" stop-color="#fff" stop-opacity="0"/>
      <stop offset="1" stop-color="#000" stop-opacity=".38"/>
    </linearGradient>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#1b2029"/>
      <stop offset="1" stop-color="#0d0f13"/>
    </linearGradient>
  </defs>

  <rect width="{W}" height="{H}" rx="112" fill="url(#bg)"/>

  <!-- glow + shadow -->
  <circle cx="290" cy="250" r="230" fill="url(#glow)"/>
  <path d="{fin_d}" fill="#000" opacity=".45" transform="translate(9 8)"/>

  <g clip-path="url(#fin)">
    {"".join(plates)}
    <rect width="{W}" height="{H}" filter="url(#blotch)"/>
    <rect width="{W}" height="{H}" filter="url(#corrode)"/>
    <rect width="{W}" height="{H}" filter="url(#grain)" opacity=".75"/>
    <rect width="{W}" height="{H}" fill="url(#shade)"/>
    <path d="{fin_d}" fill="none" stroke="#1c0802" stroke-opacity=".55" stroke-width="26" filter="url(#soft)"/>
    <!-- plate seams: dark groove + light lip -->
    {"".join(f'<path d="{d}" fill="none" stroke="#240e04" stroke-width="5"/><path d="{d}" fill="none" stroke="#f0a060" stroke-opacity=".35" stroke-width="1.5" transform="translate(0 3)"/>' for d in seam_paths)}
    {"".join(f'<path d="{d}" fill="none" stroke="#240e04" stroke-width="4"/><path d="{d}" fill="none" stroke="#f0a060" stroke-opacity=".3" stroke-width="1.2" transform="translate(2.5 0)"/>' for d in verticals)}
    {"".join(streaks)}
    {rivet_svg}
  </g>

  <!-- outline -->
  <path d="{fin_d}" fill="none" stroke="#2a1006" stroke-width="5" stroke-linejoin="round"/>

</svg>
'''

out = pathlib.Path(__file__).resolve().parent.parent / "assets" / "logo.svg"
out.parent.mkdir(exist_ok=True)
out.write_text(svg, encoding="utf-8")
print(out, len(rivets), "rivets")
