#!/bin/sh
# usage: harness/icons.sh [classic|sari|waves]
# Builds every app icon file from assets/icons/<name>/: mark.svg is the full line art on the
# 1024 tile, and mark-small.svg the bold head-and-pen crop for 48 and 64 px, where the full art
# turns gray. Sizes up to 32 px show the solid head and pen of the menu bar glyph instead. It
# also renders the menu bar and Linux tray glyphs from
# assets/hushpen-menubar.svg. classic ships; sari and waves are kept for seasonal use. Writes into
# assets/ (or $ICON_OUT). Needs rsvg-convert, ImageMagick, iconutil, and Xcode 26 or later for
# Assets.car, which macOS 26 and later draw; without it they shrink the .icns into a gray tile.
set -eu
REPO=$(cd "$(dirname "$0")/.." && pwd)
NAME=${1:-classic}
SRC=$REPO/assets/icons/$NAME
OUT=${ICON_OUT:-$REPO/assets}
[ -f "$SRC/mark.svg" ] && [ -f "$SRC/mark-small.svg" ] || { echo "no icon named $NAME in assets/icons" >&2; exit 1; }
TILE=$(cat "$REPO/assets/icons/tile.path")
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
mkdir -p "$OUT/linux-icons"

art() { sed -e '/<svg/d' -e '/<\/svg>/d' "$1"; }

# The Apple grid: an 824 px squircle at 100 px on the 1024 canvas, with the template shadow.
tile() {
  cat <<SVG
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" role="img" aria-label="Hushpen app icon">
  <defs>
    <linearGradient id="marble" x1="0" y1="100" x2="0" y2="924" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#F7F4EF"/>
      <stop offset="1" stop-color="#E8E3DA"/>
    </linearGradient>
    <clipPath id="tile"><path d="$TILE"/></clipPath>
    <filter id="shadow" x="-10%" y="-10%" width="120%" height="125%">
      <feDropShadow dx="0" dy="10" stdDeviation="5" flood-color="#000000" flood-opacity="0.3"/>
    </filter>
  </defs>
  <path d="$TILE" fill="url(#marble)" filter="url(#shadow)"/>
  <g clip-path="url(#tile)"><g transform="translate(100 100) scale(0.8046875)">
$(art "$1")
  </g></g>
  <path d="$TILE" fill="none" stroke="#000000" stroke-opacity="0.07" stroke-width="2"/>
</svg>
SVG
}

tile "$SRC/mark.svg" > "$OUT/hushpen-app-icon.svg"
tile "$SRC/mark-small.svg" > "$T/small.svg"
{
  echo '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">'
  echo '<g transform="translate(71 192) scale(20)">'
  art "$REPO/assets/hushpen-menubar.svg" | sed 's/#000000/#1B2228/'
  echo '</g>'
  echo '</svg>'
} > "$T/tiny-mark.svg"
tile "$T/tiny-mark.svg" > "$T/tiny.svg"
render() { rsvg-convert -w "$2" -h "$2" "$1" -o "$3"; }
art_for() {
  if [ "$1" -le 32 ]; then echo "$T/tiny.svg"; elif [ "$1" -le 64 ]; then echo "$T/small.svg"; else echo "$OUT/hushpen-app-icon.svg"; fi
}
render "$OUT/hushpen-app-icon.svg" 1024 "$OUT/hushpen-app-icon.png"

mkdir "$T/Hushpen.iconset"
for size in 16 32 128 256 512; do
  for scale in 1 2; do
    px=$((size * scale)); svg=$(art_for $px)
    name=icon_${size}x${size}; [ $scale = 2 ] && name=${name}@2x
    render "$svg" $px "$T/Hushpen.iconset/$name.png"
  done
done
iconutil -c icns "$T/Hushpen.iconset" -o "$OUT/hushpen-app-icon.icns"

for px in 16 24 32 48 64 128 256 512 1024; do
  render "$(art_for $px)" $px "$OUT/linux-icons/${px}x${px}.png"
done

# About draws the tile without its margin at 96 pt with a 16 pt corner, at 2x.
cat > "$T/about.svg" <<SVG
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs><clipPath id="c"><rect width="1024" height="1024" rx="171"/></clipPath></defs>
  <g clip-path="url(#c)"><rect width="1024" height="1024" fill="#F2EFEA"/>
$(art "$SRC/mark.svg")
  </g>
</svg>
SVG
render "$T/about.svg" 192 "$OUT/hushpen-app-icon-192.png"

# The Icon Composer source: the marble fill and the art as one layer. In the Dark icon style macOS
# draws its own dark tile, and the lines turn light.
ICON=$OUT/hushpen-app-icon.icon
rm -rf "$ICON"; mkdir -p "$ICON/Assets"
cp "$SRC/mark.svg" "$ICON/Assets/mark.svg"
cat > "$ICON/icon.json" <<'JSON'
{
  "fill" : {
    "linear-gradient" : [
      "srgb:0.96863,0.95686,0.93725,1.00000",
      "srgb:0.90980,0.89020,0.85490,1.00000"
    ],
    "orientation" : { "start" : { "x" : 0.5, "y" : 0 }, "stop" : { "x" : 0.5, "y" : 1 } }
  },
  "groups" : [
    {
      "layers" : [
        {
          "glass" : false,
          "image-name" : "mark.svg",
          "name" : "mark",
          "fill-specializations" : [
            { "appearance" : "dark", "value" : { "solid" : "srgb:0.92549,0.90588,0.87059,1.00000" } }
          ]
        }
      ],
      "shadow" : { "kind" : "neutral", "opacity" : 0.15 },
      "translucency" : { "enabled" : false, "value" : 0.5 }
    }
  ],
  "supported-platforms" : { "squares" : [ "macOS" ] }
}
JSON
# actool names the icon in Assets.car after the .icon folder, not after --app-icon, so the
# source is compiled under the name CFBundleIconName asks for.
cp -R "$ICON" "$T/Hushpen.icon"
mkdir "$T/car"
xcrun actool "$T/Hushpen.icon" --compile "$T/car" \
  --output-format human-readable-text --errors --warnings \
  --output-partial-info-plist "$T/partial.plist" --app-icon Hushpen --include-all-app-icons \
  --enable-on-demand-resources NO --development-region en --target-device mac \
  --minimum-deployment-target 13.0 --platform macosx >/dev/null
cp "$T/car/Assets.car" "$OUT/hushpen-app-icon.car"

# The menu bar glyph is a template image of 22 by 16 points at 2x. The Linux tray takes the
# same glyph on a 32 px square, black for light panels and white for dark ones.
rsvg-convert -w 44 -h 32 "$REPO/assets/hushpen-menubar.svg" -o "$T/menubar.png"
magick "$T/menubar.png" PNG32:"$OUT/hushpen-menubar.png"
magick "$T/menubar.png" -crop 32x32+6+0 +repage PNG32:"$OUT/hushpen-tray-dark.png"
magick "$OUT/hushpen-tray-dark.png" -fill white -colorize 100 PNG32:"$OUT/hushpen-tray-light.png"
echo "built the $NAME icon into $OUT"
