"""Builds the embedded Inter subsets from the official Inter release.

Usage (needs `pip install fonttools`):
    python subset.py <Inter-4.1/extras/ttf> <output dir>

- Keeps Basic Latin, Latin-1 and common punctuation (the English UI and the
  rhythm names). Other scripts are added when translations arrive.
- Maps the digits to their tabular (fixed-width) forms, because egui does not
  apply OpenType features: the BPM number must not change width as it changes.
- Drops hinting and layout tables egui does not use.
"""

import sys
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

WEIGHTS = ["Medium", "SemiBold"]
UNICODES = (
    list(range(0x20, 0x7F))  # Basic Latin
    + list(range(0xA0, 0x100))  # Latin-1 Supplement
    + [0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2026, 0x2212]
)


def tabular_digits(font: TTFont) -> None:
    """Points the cmap entries of 0-9 at the glyphs the `tnum` feature uses."""
    gsub = font["GSUB"].table
    singles = {}
    for record in gsub.FeatureList.FeatureRecord:
        if record.FeatureTag != "tnum":
            continue
        for index in record.Feature.LookupListIndex:
            for sub in gsub.LookupList.Lookup[index].SubTable:
                mapping = getattr(sub, "mapping", None)
                if mapping:
                    singles.update(mapping)
    for table in font["cmap"].tables:
        for code in range(0x30, 0x3A):
            glyph = table.cmap.get(code)
            if glyph in singles:
                table.cmap[code] = singles[glyph]


def main() -> None:
    src, out = Path(sys.argv[1]), Path(sys.argv[2])
    for weight in WEIGHTS:
        font = TTFont(src / f"Inter-{weight}.ttf")
        tabular_digits(font)
        options = subset.Options()
        options.layout_features = ["kern"]
        options.hinting = False
        options.desubroutinize = True
        options.name_IDs = [0, 1, 2, 3, 4, 5, 6]
        subsetter = subset.Subsetter(options)
        subsetter.populate(unicodes=UNICODES)
        subsetter.subset(font)
        target = out / f"Inter-{weight}.subset.ttf"
        font.save(target)
        print(target, target.stat().st_size)


if __name__ == "__main__":
    main()
