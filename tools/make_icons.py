#!/usr/bin/env python3
"""Generate the Incredibulk icon set.

Kept as a script rather than checked-in binaries so the mark can be adjusted
without a design tool: edit the geometry below and re-run.

    python3 tools/make_icons.py

The mark is the product in one glyph: three loose fragments stacked above one
solid bar they collapse into. It has to survive a 16 px tray, so there is no
detail below a few percent of the canvas.
"""

from __future__ import annotations

import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(os.path.dirname(HERE), "app", "icons")

# Palette.
BG_TOP = (0x1B, 0x1E, 0x3A, 255)
BG_BOTTOM = (0x0F, 0x11, 0x24, 255)
FRAGMENT = (0xDE, 0xE2, 0xFF, 255)
ACCENT = (0x7C, 0x6C, 0xFF, 255)
ACCENT_LIVE = (0x34, 0xD3, 0x99, 255)

SUPERSAMPLE = 3
MASTER = 512

# Geometry in unit coordinates: (x, y, width, height, radius, colour key).
# Three fragments of uneven width, then the block they flush into.
FRAGMENTS = [
    (0.200, 0.255, 0.360, 0.070),
    (0.200, 0.390, 0.470, 0.070),
    (0.200, 0.525, 0.300, 0.070),
]
BLOCK = (0.200, 0.680, 0.600, 0.110)


def rounded_rect_contains(px, py, x, y, w, h, r):
    """Point-in-rounded-rectangle, in unit coordinates."""
    if px < x or px > x + w or py < y or py > y + h:
        return False
    r = min(r, w / 2.0, h / 2.0)
    cx = min(max(px, x + r), x + w - r)
    cy = min(max(py, y + r), y + h - r)
    dx, dy = px - cx, py - cy
    return dx * dx + dy * dy <= r * r


def blend(dst, src, coverage):
    """Source-over with a scalar coverage factor."""
    if coverage <= 0.0:
        return dst
    sa = (src[3] / 255.0) * coverage
    if sa >= 1.0:
        return src
    out = []
    da = dst[3] / 255.0
    oa = sa + da * (1.0 - sa)
    for i in range(3):
        s = src[i] / 255.0
        d = dst[i] / 255.0
        v = (s * sa + d * da * (1.0 - sa)) / oa if oa > 0 else 0.0
        out.append(int(round(v * 255)))
    out.append(int(round(oa * 255)))
    return tuple(out)


def render(size, with_background=True, accent=ACCENT, monochrome=False):
    """Render the mark into a flat RGBA byte list."""
    ss = SUPERSAMPLE
    n = size * ss
    step = 1.0 / n
    half = step / 2.0

    shapes = []
    if with_background:
        shapes.append(("bg", 0.0, 0.0, 1.0, 1.0, 0.225))
    for (x, y, w, h) in FRAGMENTS:
        shapes.append(("fragment", x, y, w, h, h / 2.0))
    bx, by, bw, bh = BLOCK
    shapes.append(("accent", bx, by, bw, bh, bh / 2.0))

    pixels = bytearray(size * size * 4)

    for row in range(size):
        for col in range(size):
            acc = [0.0, 0.0, 0.0, 0.0]
            for sy in range(ss):
                py = (row * ss + sy) * step + half
                for sx in range(ss):
                    px = (col * ss + sx) * step + half
                    sample = (0, 0, 0, 0)
                    for kind, x, y, w, h, r in shapes:
                        if not rounded_rect_contains(px, py, x, y, w, h, r):
                            continue
                        if kind == "bg":
                            t = py
                            colour = tuple(
                                int(round(BG_TOP[i] * (1 - t) + BG_BOTTOM[i] * t))
                                for i in range(3)
                            ) + (255,)
                        elif kind == "fragment":
                            colour = (255, 255, 255, 235) if monochrome else FRAGMENT
                        else:
                            colour = (255, 255, 255, 255) if monochrome else accent
                        sample = blend(sample, colour, 1.0)
                    for i in range(4):
                        acc[i] += sample[i]
            total = float(ss * ss)
            base = (row * size + col) * 4
            for i in range(4):
                pixels[base + i] = int(round(acc[i] / total))
    return pixels


def downsample(pixels, size, target):
    """Box filter. Only ever called with size divisible by target."""
    if size == target:
        return pixels
    k = size // target
    out = bytearray(target * target * 4)
    for row in range(target):
        for col in range(target):
            acc = [0, 0, 0, 0]
            for dy in range(k):
                for dx in range(k):
                    base = ((row * k + dy) * size + (col * k + dx)) * 4
                    for i in range(4):
                        acc[i] += pixels[base + i]
            base = (row * target + col) * 4
            for i in range(4):
                out[base + i] = acc[i] // (k * k)
    return out


def png_bytes(pixels, size):
    raw = bytearray()
    stride = size * 4
    for row in range(size):
        raw.append(0)  # filter: none
        raw.extend(pixels[row * stride:(row + 1) * stride])

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def ico_bytes(entries):
    """ICO with PNG-compressed entries, supported since Windows Vista."""
    header = struct.pack("<HHH", 0, 1, len(entries))
    offset = len(header) + 16 * len(entries)
    directory, blobs = b"", b""
    for size, data in entries:
        dim = 0 if size >= 256 else size
        directory += struct.pack(
            "<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset
        )
        blobs += data
        offset += len(data)
    return header + directory + blobs


def icns_bytes(entries):
    """ICNS with PNG payloads, one block per supported size."""
    types = {16: b"icp4", 32: b"icp5", 64: b"icp6", 128: b"ic07", 256: b"ic08", 512: b"ic09"}
    body = b""
    for size, data in entries:
        tag = types.get(size)
        if tag is None:
            continue
        body += tag + struct.pack(">I", len(data) + 8) + data
    return b"icns" + struct.pack(">I", len(body) + 8) + body


def main():
    os.makedirs(OUT, exist_ok=True)
    print(f"rendering master at {MASTER}px ({SUPERSAMPLE}x supersampled)")
    master = render(MASTER)

    sizes = [512, 256, 128, 64, 32, 16]
    rendered = {s: downsample(master, MASTER, s) for s in sizes}
    png = {s: png_bytes(rendered[s], s) for s in sizes}

    files = {
        "icon.png": png[512],
        "128x128.png": png[128],
        "128x128@2x.png": png[256],
        "32x32.png": png[32],
        "icon.ico": ico_bytes([(s, png[s]) for s in (16, 32, 64, 128, 256)]),
        "icon.icns": icns_bytes([(s, png[s]) for s in (16, 32, 64, 128, 256, 512)]),
    }

    # Tray marks: no background plate, so they sit on any menu bar. The live
    # variant is what tells the user at a glance that a session is open.
    for name, accent, mono in (
        ("tray-idle.png", ACCENT, True),
        ("tray-live.png", ACCENT_LIVE, False),
    ):
        big = render(128, with_background=False, accent=accent, monochrome=mono)
        files[name] = png_bytes(downsample(big, 128, 32), 32)

    for name, data in files.items():
        path = os.path.join(OUT, name)
        with open(path, "wb") as fh:
            fh.write(data)
        print(f"  {name:20s} {len(data):>7d} bytes")


if __name__ == "__main__":
    main()
