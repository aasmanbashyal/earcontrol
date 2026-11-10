# /// script
# requires-python = ">=3.11"
# dependencies = ["fonttools==4.63.0"]
# ///
"""Build the small emergency font from the verified egui 0.33.3 Hack font.

Run with: uv run scripts/subset-emergency-font.py PATH/TO/Hack-Regular.ttf
Windows system fonts provide the normal UI and wider Unicode coverage.
"""

import argparse
import hashlib
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

SOURCE_SHA256 = "15f55cc0c85a2988d2b4b3a8cdb5d77fdfbaf319e1bb5309d725db9818fb7125"
UNICODES = set(range(0x20, 0x7F)) | set(range(0xA0, 0x100))
UNICODES |= set(range(0x2010, 0x203B)) | set(range(0x2190, 0x219A))
UNICODES |= {0x2212, 0x25A0, 0x25A1, 0x25B2, 0x25B6, 0x25BC, 0x25C0, 0x25CB, 0x25CF, 0x2713}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "desktop/assets/fonts/EmergencyMono.ttf",
    )
    args = parser.parse_args()
    source = args.source.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise SystemExit("Source hash differs from egui 0.33.3 Hack-Regular.ttf.")

    options = subset.Options()
    options.hinting = False
    options.notdef_glyph = True
    options.notdef_outline = True
    options.recommended_glyphs = True
    options.layout_features = []
    options.name_IDs = [0, 1, 2, 3, 4, 5, 6, 8, 9, 13, 14, 16, 17]
    options.name_legacy = True
    options.name_languages = ["*"]
    options.recalc_timestamp = False
    font = TTFont(args.source, recalcTimestamp=False)
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=UNICODES)
    subsetter.subset(font)
    renamed = {
        1: "EarControl Emergency Mono",
        2: "Regular",
        3: "EarControlEmergencyMono-Regular-1.0",
        4: "EarControl Emergency Mono Regular",
        5: "Version 1.000",
        6: "EarControlEmergencyMono-Regular",
        16: "EarControl Emergency Mono",
        17: "Regular",
    }
    for record in font["name"].names:
        if record.nameID in renamed:
            record.string = renamed[record.nameID].encode(record.getEncoding())
    # Keep timestamps deterministic rather than embedding the generation date.
    font["head"].created = 3_406_620_153
    font["head"].modified = 3_406_620_153
    args.output.parent.mkdir(parents=True, exist_ok=True)
    font.save(args.output, reorderTables=True)
    font.close()

    with TTFont(args.output) as check:
        covered = set(check.getBestCmap())
        required = set(range(0x20, 0x7F)) | {ord(c) for c in "·›…●←→↗"}
        if not required <= covered:
            raise SystemExit(f"Missing required fallback characters: {required - covered}")
    data = args.output.read_bytes()
    print(f"{args.output.name}: {len(data)} bytes; SHA256 {hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
