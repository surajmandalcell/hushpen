"""Static preview of site/public. Serves /page from page.html, as Cloudflare does live.
usage: python3 site/preview.py [port]"""
import http.server, os, sys
from functools import partial

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "public")


class Handler(http.server.SimpleHTTPRequestHandler):
    def translate_path(self, path):
        p = super().translate_path(path)
        return p + ".html" if not os.path.exists(p) and os.path.isfile(p + ".html") else p


port = int(sys.argv[1]) if len(sys.argv) > 1 else 4821
http.server.ThreadingHTTPServer(("127.0.0.1", port), partial(Handler, directory=ROOT)).serve_forever()
