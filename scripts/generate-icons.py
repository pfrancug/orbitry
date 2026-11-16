#!/usr/bin/env python3
"""Generate Orbitry tray, exe, favicon, and sidebar icons from the SVG mark.

The mark is a flat rounded square with a planet and a moon. Geometry lives on a
32-unit grid so 16px and 32px stay pixel-aligned. Run from the repo root:

    python scripts/generate-icons.py
"""

from __future__ import annotations

import io
import struct
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:
    sys.stderr.write("This script needs Pillow: pip install pillow\n")
    raise SystemExit(1)

# Must stay in sync with assets/orbitry.svg (viewBox 0 0 32 32).
SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" role="img" aria-label="Orbitry">
  <rect width="32" height="32" rx="8" fill="#8aa4ff"/>
  <circle cx="14" cy="18" r="6" fill="#f5f5f8"/>
  <circle cx="22" cy="10" r="4" fill="#c4b5fd"/>
</svg>
"""

BG = (0x8A, 0xA4, 0xFF, 255)
PLANET = (0xF5, 0xF5, 0xF8, 255)
MOON = (0xC4, 0xB5, 0xFD, 255)


def repo_root() -> Path:
    here = Path(__file__).resolve()
    if here.parent.name == "scripts":
        return here.parent.parent
    return Path.cwd()


def render(size: int) -> Image.Image:
    if size >= 64:
        ss = 8
        canvas = size * ss
        img = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
        draw = ImageDraw.Draw(img)
        scale = canvas / 32
        draw.rounded_rectangle(
            [0, 0, canvas - 1, canvas - 1], radius=8 * scale, fill=BG
        )
        def circle(cx: float, cy: float, r: float, fill: tuple[int, int, int, int]) -> None:
            draw.ellipse(
                [(cx - r) * scale, (cy - r) * scale, (cx + r) * scale - 1, (cy + r) * scale - 1],
                fill=fill,
            )
        circle(14, 18, 6, PLANET)
        circle(22, 10, 4, MOON)
        return img.resize((size, size), Image.Resampling.LANCZOS)

    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    scale = size / 32
    draw.rounded_rectangle([0, 0, size - 1, size - 1], radius=round(8 * scale), fill=BG)

    def circle(cx: float, cy: float, r: float, fill: tuple[int, int, int, int]) -> None:
        draw.ellipse(
            [scale * (cx - r), scale * (cy - r), scale * (cx + r) - 1, scale * (cy + r) - 1],
            fill=fill,
        )

    circle(14, 18, 6, PLANET)
    circle(22, 10, 4, MOON)
    return img


def write_ico(path: Path, images: list[Image.Image]) -> None:
    """Write a PNG-compressed ICO (supported since Windows Vista)."""
    entries: list[tuple[int, int, int, int]] = []
    blobs: list[bytes] = []
    offset = 6 + 16 * len(images)
    for image in images:
        buf = io.BytesIO()
        image.save(buf, format="PNG")
        data = buf.getvalue()
        width, height = image.size
        entries.append((0 if width >= 256 else width, 0 if height >= 256 else height, len(data), offset))
        blobs.append(data)
        offset += len(data)
    raw = bytearray()
    raw += struct.pack("<HHH", 0, 1, len(images))
    for width, height, size, off in entries:
        raw += struct.pack("<BBBBHHII", width, height, 0, 0, 1, 32, size, off)
    for blob in blobs:
        raw += blob
    path.write_bytes(raw)


def main() -> None:
    root = repo_root()
    assets = root / "crates" / "server" / "assets"
    public = root / "ui" / "public"
    assets.mkdir(parents=True, exist_ok=True)
    public.mkdir(parents=True, exist_ok=True)

    rasters = {size: render(size) for size in (16, 24, 32, 48, 64, 128, 180, 256)}

    (assets / "orbitry.svg").write_text(SVG, encoding="utf-8")
    rasters[256].save(assets / "orbitry.png")
    write_ico(assets / "orbitry.ico", [rasters[size] for size in (16, 24, 32, 48, 64, 256)])
    (assets / "tray-32.rgba").write_bytes(rasters[32].tobytes())

    (public / "favicon.svg").write_text(SVG, encoding="utf-8")
    (public / "orbitry.svg").write_text(SVG, encoding="utf-8")
    rasters[32].save(public / "favicon-32.png")
    rasters[180].save(public / "apple-touch-icon.png")
    write_ico(public / "favicon.ico", [rasters[size] for size in (16, 32, 48)])

    print("Wrote icons from the Orbitry SVG mark:")
    for path in (
        assets / "orbitry.svg",
        assets / "orbitry.png",
        assets / "orbitry.ico",
        assets / "tray-32.rgba",
        public / "orbitry.svg",
        public / "favicon.svg",
        public / "favicon-32.png",
        public / "favicon.ico",
        public / "apple-touch-icon.png",
    ):
        print(f"  {path.relative_to(root)} ({path.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
