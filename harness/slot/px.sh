#!/bin/bash
# usage: px.sh <png> x,y [x,y ...]   prints "x,y #RRGGBB" for each absolute pixel
file=$1
shift
for point in "$@"; do
  x=${point%,*}
  y=${point#*,}
  hex=$(convert "$file" -crop 1x1+"$x"+"$y" +repage -depth 8 -format '%[hex:u.p{0,0}]' info:- 2>/dev/null)
  echo "$x,$y #${hex:0:6}"
done
