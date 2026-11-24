#!/bin/sh
# The README's captures of the MURB report page. Run from the repository root:
#   sh docs/media/murb-report.sh
# Needs Google Chrome (or CHROME=/path/to/chrome) and ffmpeg. Renders
# examples/MURB/Ifc4_ARC-in-Toronto/murb_results.html 1200 px wide at 2x in
# both colour schemes and crops three parts of it: the top (title, compass,
# intensities), the year with where the heat goes, and the envelope. The crop
# boxes follow the page's section edges at that width (CSS px: x 56-1144,
# y 8-788, 806-1830 and 1849-2465); move them if the page's layout changes.
set -eu

chrome=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
page="file://$PWD/examples/MURB/Ifc4_ARC-in-Toronto/murb_results.html"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Blink's preferredColorScheme: 0 is dark, 1 is light.
for scheme in dark:0 light:1; do
  name=${scheme%%:*}
  "$chrome" --headless=new --disable-gpu --hide-scrollbars \
    --force-device-scale-factor=2 --window-size=1200,2960 \
    --virtual-time-budget=4000 --blink-settings=preferredColorScheme=${scheme##*:} \
    --screenshot="$tmp/$name.png" "$page" 2>/dev/null
  for part in report:1560:16 year:2048:1612 envelope:1232:3698; do
    IFS=: read -r what height top <<CROP
$part
CROP
    ffmpeg -v error -y -i "$tmp/$name.png" -vf "crop=2176:$height:112:$top" \
      -compression_level 100 -pred mixed "docs/media/murb-$what-$name.png"
  done
done
