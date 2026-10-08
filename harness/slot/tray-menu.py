#!/usr/bin/env python3
"""Reads and clicks the tray menu of the Hushpen StatusNotifier item over D-Bus.

The item is the one the watcher stub (sni-host.py) lists. Its menu is a com.canonical.dbusmenu
object; a click is the `clicked` event for the item's id, which is what a desktop panel sends.

usage:
  tray-menu.py item            print the item's bus name and path
  tray-menu.py props           print Id, Title, and the size of the icon pixmap as JSON
  tray-menu.py labels          print the menu labels, one per line, in order
  tray-menu.py click <label>   send `clicked` for the entry with that label
"""
import json
import subprocess
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

ITEM_IFACE = "org.kde.StatusNotifierItem"
MENU_IFACE = "com.canonical.dbusmenu"
PROPS = "org.freedesktop.DBus.Properties"


def bus():
    return Gio.bus_get_sync(Gio.BusType.SESSION, None)


def find_item():
    out = subprocess.run(
        [sys.executable, "/harness/slot/sni-host.py", "--items"],
        capture_output=True, text=True,
    )
    lines = [line for line in out.stdout.split("\n") if line]
    if len(lines) != 1:
        raise SystemExit(f"expected one tray item, found {len(lines)}: {lines}")
    name, _, path = lines[0].partition("/")
    return name, "/" + path


def call(name, path, iface, method, params, reply):
    return bus().call_sync(
        name, path, iface, method, params, GLib.VariantType(reply),
        Gio.DBusCallFlags.NONE, 3000, None,
    )


def prop(name, path, iface, key):
    return call(name, path, PROPS, "Get", GLib.Variant("(ss)", (iface, key)), "(v)").unpack()[0]


def entries(name):
    # (id, properties, children) with every property, two levels deep.
    layout = call(
        name, "/Menu", MENU_IFACE, "GetLayout", GLib.Variant("(iias)", (0, 2, [])),
        "(u(ia{sv}av))",
    ).unpack()[1]
    return [(child[0], child[1]) for child in layout[2]]


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    name, path = find_item()
    if command == "item":
        print(name, path)
    elif command == "props":
        pixmaps = prop(name, path, ITEM_IFACE, "IconPixmap")
        print(json.dumps({
            "id": prop(name, path, ITEM_IFACE, "Id"),
            "title": prop(name, path, ITEM_IFACE, "Title"),
            "icon_name": prop(name, path, ITEM_IFACE, "IconName"),
            "pixmaps": [[p[0], p[1], len(p[2])] for p in pixmaps],
        }))
    elif command == "labels":
        for _, props in entries(name):
            print(props.get("label", "-"))
    elif command == "click" and len(sys.argv) == 3:
        for item_id, props in entries(name):
            if props.get("label") == sys.argv[2]:
                call(
                    name, "/Menu", MENU_IFACE, "Event",
                    GLib.Variant("(isvu)", (item_id, "clicked", GLib.Variant("i", 0), 0)), "()",
                )
                print(f"clicked {sys.argv[2]!r} (id {item_id})")
                return
        raise SystemExit(f"no menu entry {sys.argv[2]!r}")
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
