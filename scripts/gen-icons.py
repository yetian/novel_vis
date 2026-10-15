#!/usr/bin/env python3
"""生成 ChronoSpace Reader 的占位图标（PNG / ICO / ICNS）。

生产环境应替换为真实设计稿，并使用 `npm run tauri icon path/to/icon.png`
一次性生成全平台图标。此脚本仅依赖 Python 标准库。
"""
import os
import struct
import zlib

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "src-tauri", "icons")

# 品牌色：墨黑底 + 青玉主体
BG = (11, 14, 20, 255)
FG = (32, 201, 151, 255)
ACCENT = (240, 180, 41, 255)


def make_pixels(size):
    """绘制一个极简的圆形时空标记。"""
    px = bytearray()
    cx = cy = (size - 1) / 2.0
    r_outer = size * 0.42
    r_inner = size * 0.18
    for y in range(size):
        for x in range(size):
            dx, dy = x - cx, y - cy
            dist = (dx * dx + dy * dy) ** 0.5
            if dist <= r_inner:
                px.extend(FG)
            elif dist <= r_outer:
                # 环上按角度交替颜色，形成时间刻度
                angle = int(((dx == 0 and dy == 0) and 0 or (__import__("math").atan2(dy, dx))) * 8 / 3.14159)
                color = ACCENT if angle % 2 == 0 else FG
                px.extend(color)
            else:
                px.extend(BG)
    return bytes(px)


def png_bytes(size):
    raw = make_pixels(size)
    rows = b"".join(b"\x00" + raw[y * size * 4:(y + 1) * size * 4] for y in range(size))

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b"")


def ico_bytes(sizes):
    images = [(s, png_bytes(s)) for s in sizes]
    header = struct.pack("<HHH", 0, 1, len(images))
    entries = b""
    offset = 6 + 16 * len(images)
    for s, data in images:
        dim = 0 if s >= 256 else s
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return header + entries + b"".join(d for _, d in images)


def icns_bytes(sizes):
    type_map = {16: b"icp4", 32: b"icp5", 128: b"ic07", 256: b"ic08", 512: b"ic09"}
    body = b""
    for s in sizes:
        data = png_bytes(s)
        body += type_map[s] + struct.pack(">I", len(data) + 8) + data
    return b"icns" + struct.pack(">I", len(body) + 8) + body


def write(name, data):
    path = os.path.join(OUT, name)
    with open(path, "wb") as fh:
        fh.write(data)
    print(f"  wrote {name} ({len(data)} bytes)")


def main():
    os.makedirs(OUT, exist_ok=True)
    print("Generating icons ->", OUT)
    write("32x32.png", png_bytes(32))
    write("128x128.png", png_bytes(128))
    write("128x128@2x.png", png_bytes(256))
    write("icon.png", png_bytes(512))
    write("icon.ico", ico_bytes([32, 256]))
    write("icon.icns", icns_bytes([128, 256, 512]))


if __name__ == "__main__":
    main()
