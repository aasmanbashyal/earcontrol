# Emergency font

`EmergencyMono.ttf` is a 34,388-byte subset of Hack Regular. It keeps the interface readable when Windows fonts are unavailable. Normal installations use Segoe UI, Segoe UI Semibold and Consolas, with Windows fonts for wider Unicode coverage. Windows fonts are read from the operating system and are not redistributed.

The subset contains printable ASCII, Latin-1, common punctuation, arrows and interface symbols present in the original font. It is renamed **EarControl Emergency Mono**. It is not intended to replace the full Windows fallback fonts.

Source: `fonts/Hack-Regular.ttf` from `epaint_default_fonts` 0.33.3, whose recorded egui revision is `44cdd653e2317d300fb8a6c9c36b03f23991e803`. The [original font](https://github.com/emilk/egui/blob/44cdd653e2317d300fb8a6c9c36b03f23991e803/crates/epaint_default_fonts/fonts/Hack-Regular.ttf) has SHA-256 `15f55cc0c85a2988d2b4b3a8cdb5d77fdfbaf319e1bb5309d725db9818fb7125`.

The original MIT and Bitstream Vera notices are preserved in [LICENSE-Hack.txt](LICENSE-Hack.txt) and included in the aggregate [THIRD-PARTY-NOTICES.txt](../THIRD-PARTY-NOTICES.txt) distributed with the app. The modified font's names do not contain the reserved names Bitstream or Vera.

From the repository root, using the original font from the pinned crate archive or Cargo cache:

```powershell
uv run scripts/subset-emergency-font.py 'PATH/TO/epaint_default_fonts-0.33.3/fonts/Hack-Regular.ttf'
```

The script pins fontTools 4.63.0, verifies the input hash, strips hinting, renames the family, fixes timestamps, and verifies the essential UI glyphs. Regeneration requires Python tooling; normal Cargo builds use the committed subset. Expected output SHA-256: `44132a4276435e69accc09f58460c85f5931f8bc126e89dbbdf716b271b945af`.
