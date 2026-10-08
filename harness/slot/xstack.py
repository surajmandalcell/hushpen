#!/usr/bin/env python3
"""Reads the X stacking order of the root's children.

usage: xstack.py find X Y W H     prints the id of the mapped child of the root at that geometry
       xstack.py above A B        exit 0 when the top-level window holding A is stacked above the
                                  one holding B (XQueryTree lists children bottom to top)
"""
import sys

from Xlib import display

d = display.Display()
root = d.screen().root


def top_level(wid):
    window = d.create_resource_object("window", wid)
    while True:
        parent = window.query_tree().parent
        if parent.id == root.id:
            return window.id
        window = parent


def find(x, y, w, h):
    for child in reversed(root.query_tree().children):
        try:
            geometry = child.get_geometry()
            if child.get_attributes().map_state != 2:
                continue
        except Exception:
            continue
        if (geometry.x, geometry.y, geometry.width, geometry.height) == (x, y, w, h):
            print(child.id)
            return 0
    return 1


def above(a, b):
    order = [child.id for child in root.query_tree().children]
    try:
        return 0 if order.index(top_level(a)) > order.index(top_level(b)) else 1
    except ValueError:
        return 2


if sys.argv[1] == "find":
    sys.exit(find(*map(int, sys.argv[2:6])))
sys.exit(above(int(sys.argv[2], 0), int(sys.argv[3], 0)))
