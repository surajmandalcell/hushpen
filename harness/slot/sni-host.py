#!/usr/bin/env python3
"""StatusNotifier watcher and host stub for the harness.

The slot has no desktop shell, so nothing owns org.kde.StatusNotifierWatcher. This owns it,
reports a registered host, and keeps the list of tray items an app registers, so a tray check can
tell that the app showed an icon.

usage:
  sni-host.py            run the watcher (init.sh starts this)
  sni-host.py --query    exit 0 when the watcher answers and reports a host
  sni-host.py --items    print the registered item services, one per line
"""
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

WATCHER = "org.kde.StatusNotifierWatcher"
PATH = "/StatusNotifierWatcher"
PROPS = "org.freedesktop.DBus.Properties"
INTROSPECTION = """
<node>
  <interface name="org.kde.StatusNotifierWatcher">
    <method name="RegisterStatusNotifierItem"><arg type="s" direction="in"/></method>
    <method name="RegisterStatusNotifierHost"><arg type="s" direction="in"/></method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
    <signal name="StatusNotifierItemUnregistered"><arg type="s"/></signal>
    <signal name="StatusNotifierHostRegistered"/>
    <signal name="StatusNotifierHostUnregistered"/>
  </interface>
</node>
"""


def log(message):
    print(message, flush=True)


def get_property(name):
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    reply = bus.call_sync(
        WATCHER, PATH, PROPS, "Get", GLib.Variant("(ss)", (WATCHER, name)),
        GLib.VariantType("(v)"), Gio.DBusCallFlags.NONE, 3000, None,
    )
    return reply.unpack()[0]


def query():
    try:
        return 0 if get_property("IsStatusNotifierHostRegistered") else 1
    except GLib.Error:
        return 1


def items():
    try:
        for item in get_property("RegisteredStatusNotifierItems"):
            print(item)
        return 0
    except GLib.Error as error:
        print(f"no StatusNotifier watcher: {error.message}", file=sys.stderr)
        return 1


class Watcher:
    def __init__(self):
        self.items = {}
        self.connection = None

    def on_bus(self, connection, _name):
        self.connection = connection
        node = Gio.DBusNodeInfo.new_for_xml(INTROSPECTION)
        connection.register_object(
            PATH, node.interfaces[0], self.on_call, self.on_get, None
        )

    def on_get(self, _conn, _sender, _path, _iface, name):
        if name == "RegisteredStatusNotifierItems":
            return GLib.Variant("as", sorted(self.items.values()))
        if name == "IsStatusNotifierHostRegistered":
            return GLib.Variant("b", True)
        if name == "ProtocolVersion":
            return GLib.Variant("i", 0)
        return None

    def emit(self, signal, args):
        self.connection.emit_signal(
            None, PATH, WATCHER, signal, args
        )

    def on_call(self, _conn, sender, _path, _iface, method, params, invocation):
        if method == "RegisterStatusNotifierHost":
            log(f"host registered by {sender}")
            self.emit("StatusNotifierHostRegistered", None)
            invocation.return_value(None)
            return
        service = params.unpack()[0]
        # Items give either a bus name or an object path on the caller's own name.
        full = f"{sender}{service}" if service.startswith("/") else f"{service}/StatusNotifierItem"
        owner = sender if service.startswith("/") else service
        self.items[owner] = full
        log(f"item registered: {full}")
        Gio.bus_watch_name_on_connection(
            self.connection, owner, Gio.BusNameWatcherFlags.NONE, None,
            lambda _c, name: self.on_gone(name),
        )
        self.emit("StatusNotifierItemRegistered", GLib.Variant("(s)", (full,)))
        invocation.return_value(None)

    def on_gone(self, name):
        full = self.items.pop(name, None)
        if full:
            log(f"item gone: {full}")
            self.emit("StatusNotifierItemUnregistered", GLib.Variant("(s)", (full,)))


def serve():
    watcher = Watcher()
    Gio.bus_own_name(
        Gio.BusType.SESSION, WATCHER, Gio.BusNameOwnerFlags.NONE,
        watcher.on_bus, lambda _c, _n: log(f"owning {WATCHER}"),
        lambda _c, _n: (log("lost the watcher name"), sys.exit(1)),
    )
    GLib.MainLoop().run()


if __name__ == "__main__":
    if "--query" in sys.argv:
        sys.exit(query())
    if "--items" in sys.argv:
        sys.exit(items())
    serve()
