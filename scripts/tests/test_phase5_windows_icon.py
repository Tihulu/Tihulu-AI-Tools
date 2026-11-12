"""QA-WIN-01: standard-library-only Windows Tauri icon/resource regression.

Verifies the checked-in project artwork and a well-formed multi-resolution ICO,
not Windows compilation, signer identity, installer behavior, or GUI startup.
Run with: python -m unittest discover -s scripts/tests -p "test_phase5_windows_icon.py" -v
"""
import hashlib
import json
from pathlib import Path
import struct
import tomllib
import unittest
import zlib

ROOT = Path(__file__).resolve().parents[2]
TAURI = ROOT / "apps/tihulu-ai-desktop/src-tauri"
ICO = TAURI / "icons/icon.ico"
PNG = TAURI / "icons/icon.png"
ICO_SHA256 = "73f9ed4c079e9aebe9e0cbf7a408d8f1bc053907c389574bd0b57a8865121d8f"
ORIGINAL_PNG_SHA256 = "e954deef617da893fc192127e1c251b3d8f6fe738cdd4e886c4db8f72adda54e"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
RESOLUTIONS = {(16, 16), (32, 32), (64, 64), (256, 256)}


def read_png_rgba(payload):
    """Decode 8-bit RGBA PNG using stdlib, checking chunk CRC and row filters."""
    if not payload.startswith(PNG_SIGNATURE):
        raise ValueError("Not a PNG")
    at = len(PNG_SIGNATURE)
    idat, dimensions, ended = [], None, False
    while at + 12 <= len(payload):
        length = struct.unpack_from(">I", payload, at)[0]
        if length > 8 * 1024 * 1024 or at + 12 + length > len(payload):
            raise ValueError("Invalid PNG chunk length")
        tag = payload[at + 4:at + 8]
        value = payload[at + 8:at + 8 + length]
        saved_crc = struct.unpack_from(">I", payload, at + 8 + length)[0]
        if zlib.crc32(tag + value) != saved_crc:
            raise ValueError("Invalid PNG CRC")
        at += length + 12
        if tag == b"IHDR":
            if dimensions is not None or length != 13:
                raise ValueError("Invalid IHDR")
            width, height, depth, mode, compression, filtering, interlace = struct.unpack(
                ">IIBBBBB", value
            )
            if (width < 1 or height < 1 or width > 256 or height > 256 or
                    (depth, mode, compression, filtering, interlace) != (8, 6, 0, 0, 0)):
                raise ValueError("PNG must be noninterlaced 8-bit RGBA")
            dimensions = (width, height)
        elif tag == b"IDAT":
            idat.append(value)
        elif tag == b"IEND":
            ended = True
            break
    if dimensions is None or not ended or at != len(payload):
        raise ValueError("Incomplete PNG")
    w, h = dimensions
    stride = w * 4
    decompressed = zlib.decompress(b"".join(idat))
    if len(decompressed) != h * (stride + 1):
        raise ValueError("Unexpected PNG pixel data size")
    rows = bytearray()
    previous = bytearray(stride)
    for y in range(h):
        scanline = decompressed[y * (stride + 1):(y + 1) * (stride + 1)]
        kind = scanline[0]
        row = bytearray(scanline[1:])
        if kind not in (0, 1, 2, 3, 4):
            raise ValueError("Unsupported PNG filter")
        for x in range(stride):
            a = row[x - 4] if x >= 4 else 0
            b = previous[x]
            c = previous[x - 4] if x >= 4 else 0
            if kind == 1:
                prediction = a
            elif kind == 2:
                prediction = b
            elif kind == 3:
                prediction = (a + b) // 2
            elif kind == 4:
                p = a + b - c
                da, db, dc = abs(p - a), abs(p - b), abs(p - c)
                prediction = a if da <= db and da <= dc else b if db <= dc else c
            else:
                prediction = 0
            row[x] = (row[x] + prediction) & 255
        rows.extend(row)
        previous = row
    return dimensions, bytes(rows)


def ico_entries(raw):
    if len(raw) < 6:
        raise ValueError("ICO too short")
    reserved, filetype, count = struct.unpack_from("<HHH", raw, 0)
    if (reserved, filetype) != (0, 1) or count not in range(1, 20):
        raise ValueError("Invalid ICO header")
    directory_end = 6 + 16 * count
    if directory_end > len(raw):
        raise ValueError("Truncated ICO directory")
    entries = []
    ranges = []
    for index in range(count):
        w, h, palette, zero, planes, bpp, size, offset = struct.unpack_from(
            "<BBBBHHII", raw, 6 + 16 * index
        )
        width, height = w or 256, h or 256
        if (palette, zero, planes, bpp) != (0, 0, 0, 32):
            raise ValueError("Unexpected ICO RGBA directory metadata")
        if size < 33 or offset < directory_end or offset + size > len(raw):
            raise ValueError("Out-of-bounds ICO image")
        ranges.append((offset, offset + size))
        embedded = raw[offset:offset + size]
        png_size, rgba = read_png_rgba(embedded)
        if png_size != (width, height):
            raise ValueError("ICO entry disagrees with embedded PNG dimensions")
        entries.append(((width, height), rgba))
    ranges.sort()
    if any(before[1] > after[0] for before, after in zip(ranges, ranges[1:])):
        raise ValueError("Overlapping ICO entries")
    if ranges[-1][1] != len(raw):
        raise ValueError("Unexpected trailing ICO data")
    return entries


class WindowsTauriIconTests(unittest.TestCase):
    def test_authorized_project_png_is_unchanged(self):
        self.assertTrue(PNG.is_file())
        self.assertEqual(hashlib.sha256(PNG.read_bytes()).hexdigest(), ORIGINAL_PNG_SHA256)
        size, pixels = read_png_rgba(PNG.read_bytes())
        self.assertEqual(size, (64, 64))
        self.assertEqual(len(pixels), 64 * 64 * 4)

    def test_windows_icon_has_all_intended_rgba_resolutions(self):
        self.assertTrue(ICO.is_file(), "Tauri Windows resource requires icons/icon.ico")
        raw = ICO.read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), ICO_SHA256)
        entries = ico_entries(raw)
        self.assertEqual({size for size, _ in entries}, RESOLUTIONS)
        self.assertEqual(len(entries), len(RESOLUTIONS))

    def test_embedded_64_preserves_authorized_artwork_after_resampling(self):
        expected_size, source_pixels = read_png_rgba(PNG.read_bytes())
        actual_size, icon_pixels = next(
            (size, pixels) for size, pixels in ico_entries(ICO.read_bytes())
            if size == (64, 64)
        )
        self.assertEqual(actual_size, expected_size)
        self.assertEqual(len(icon_pixels), len(source_pixels))
        # ICO generation resizes the authorized PNG via Pillow resampling;
        # slight antialiasing differences are expected, exact icon SHA is pinned.
        deviations = [abs(a - b) for a, b in zip(source_pixels, icon_pixels)]
        self.assertLess(sum(deviations) / len(deviations), 2.0)
        self.assertLessEqual(max(deviations), 32)

    def test_corrupted_resource_fails_closed(self):
        bad = bytearray(ICO.read_bytes())
        bad[-15] ^= 0x40
        with self.assertRaises(ValueError):
            ico_entries(bytes(bad))

    def test_tauri_windows_build_resource_and_restrictive_csp(self):
        with (TAURI / "Cargo.toml").open("rb") as stream:
            cargo = tomllib.load(stream)
        self.assertEqual(cargo["build-dependencies"]["tauri-build"]["version"], "2")
        self.assertEqual(cargo["dependencies"]["tauri"]["version"], "2")
        self.assertIn("tauri_build::build()", (TAURI / "build.rs").read_text(encoding="utf-8"))
        config = json.loads((TAURI / "tauri.conf.json").read_text(encoding="utf-8"))
        self.assertIs(config["bundle"]["active"], False)
        self.assertEqual(config["build"]["frontendDist"], "../../tihulu-ai-web")
        csp = config["app"]["security"]["csp"]
        for requirement in (
            "default-src 'self'", "script-src 'self'",
            "connect-src 'none'", "object-src 'none'",
            "frame-src 'none'", "base-uri 'none'",
            "form-action 'none'",
        ):
            self.assertIn(requirement, csp)
        self.assertFalse(config.get("build", {}).get("beforeBuildCommand"))
        self.assertFalse(config.get("build", {}).get("beforeDevCommand"))
        self.assertNotIn("allowlist", config)
        with (ROOT / "Cargo.toml").open("rb") as stream:
            workspace = tomllib.load(stream)["workspace"]
        self.assertIn("apps/tihulu-ai-desktop/src-tauri", workspace["exclude"])


if __name__ == "__main__":
    unittest.main()
