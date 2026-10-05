#!/bin/bash
# usage: targets.sh [--stock]
# Starts the paste targets and prints their window ids:
#   xterm    `cat > /out/xterm.txt`. Stock xterm has no Ctrl+Shift+V, so a translation override
#            maps it to the CLIPBOARD; Shift+Insert keeps its stock PRIMARY paste.
#   gtk      one Entry; Enter writes /out/gtk.txt and prints GTK_ENTRY=<text>.
#   --stock  also starts an xterm without the override (`cat > /out/xterm-stock.txt`), for the
#            case that expects the Shift+Insert path only.
# Run through with-env.sh (the host script `xdo.sh` and the suites do).
rm -f /out/xterm.txt /out/xterm-stock.txt /out/gtk.txt
pkill -x xterm 2>/dev/null
pkill -f target-gtk.py 2>/dev/null
sleep 0.2

xterm -T hushpen-xterm -geometry 60x6+40+20 \
  -xrm 'XTerm*VT100.translations: #override Ctrl Shift <Key>V: insert-selection(CLIPBOARD)' \
  -e sh -c 'stty -echo; cat > /out/xterm.txt' >/logs/xterm.log 2>&1 &
python3 /harness/slot/target-gtk.py >/out/gtk-stdout.log 2>/logs/gtk-stderr.log &
if [ "${1:-}" = "--stock" ]; then
  xterm -T hushpen-xterm-stock -geometry 60x6+40+200 \
    -e sh -c 'stty -echo; cat > /out/xterm-stock.txt' >/logs/xterm-stock.log 2>&1 &
fi

XT=$(timeout 10 xdotool search --sync --onlyvisible --name '^hushpen-xterm$' | head -1)
GT=$(timeout 10 xdotool search --sync --onlyvisible --name hushpen-gtk-target | head -1)
echo "xterm=$XT gtk=$GT"
if [ "${1:-}" = "--stock" ]; then
  ST=$(timeout 10 xdotool search --sync --onlyvisible --name hushpen-xterm-stock | head -1)
  echo "xterm_stock=$ST"
fi
[ -n "$XT" ] && [ -n "$GT" ]
