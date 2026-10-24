"""Loopback-only server for the explicit synthetic OPFS browser verification."""
import http.server
import pathlib
import sys
import os
import subprocess


def main():
    workspace=pathlib.Path(os.environ.get("BUILD_WORKSPACE_DIRECTORY", pathlib.Path.cwd()))
    revision=subprocess.check_output(["git","rev-parse","HEAD"],cwd=workspace,text=True).strip()
    if subprocess.check_output(["git","status","--porcelain"],cwd=workspace,text=True).strip():
        raise SystemExit("browser verification requires a clean revision")
    port=int(sys.argv[1])
    files={}
    for raw in sys.argv[2:]:
        path=pathlib.Path(raw).resolve()
        files['/'+path.name]=path
    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            route=self.path.split('?',1)[0]
            if route=='/':route='/index.html'
            path=files.get(route)
            if not path:
                self.send_error(404);return
            data=path.read_bytes()
            mime='application/wasm' if path.suffix=='.wasm' else 'text/javascript' if path.suffix=='.mjs' else 'application/json' if path.suffix=='.json' else 'text/html'
            self.send_response(200);self.send_header('Content-Type',mime)
            self.send_header('Content-Length',str(len(data)));self.send_header('Cache-Control','no-store');self.end_headers();self.wfile.write(data)
        def log_message(self,*args):pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',port),Handler)
    print(f'OPFS synthetic verification: http://127.0.0.1:{port}/ at {revision}',flush=True)
    server.serve_forever()


if __name__=='__main__':main()
