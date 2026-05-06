#!/usr/bin/env bash
# gen_placeholder_icons.sh — Generate 32×32 placeholder PNG icons for Feature #12.
#
# Run once from the project root: bash scripts/gen_placeholder_icons.sh
# Re-runnable; idempotent (existing files are overwritten with identical content).
#
# Requires: ImageMagick v7 `magick` (brew install imagemagick on macOS).
# Generated files: assets/ui/icons/items/*.png (8 files, <1 KB each)
#
# Note: Text annotation requires a system font. Solid-color blocks are used
# as placeholders so the script is portable without font configuration.
# Feature #25 (UI) will replace these with real icons.
#
# Color scheme (distinct per item kind):
#   Weapons    — dark red    (#a03030): rusty_sword, oak_staff, wooden_mace
#   Armor      — brown       (#806040): leather_armor, robe
#   Shield     — dark green  (#507050): wooden_shield
#   Consumable — teal        (#308050): healing_potion
#   Key item   — dark gold   (#a08020): rusty_key

set -euo pipefail

ICONS_DIR="assets/ui/icons/items"
mkdir -p "${ICONS_DIR}"

generate() {
    local color="$1"
    local outfile="${ICONS_DIR}/$2"
    magick -size 32x32 xc:"${color}" "${outfile}"
}

# Weapons (dark red)
generate '#a03030' rusty_sword.png
generate '#a03030' oak_staff.png
generate '#a03030' wooden_mace.png

# Armor (brown)
generate '#806040' leather_armor.png
generate '#806040' robe.png

# Shield (dark green)
generate '#507050' wooden_shield.png

# Consumable (teal)
generate '#308050' healing_potion.png

# Key item (dark gold)
generate '#a08020' rusty_key.png

echo "Generated 8 placeholder icons in ${ICONS_DIR}/"
