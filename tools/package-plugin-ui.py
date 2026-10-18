#!/usr/bin/env python3
"""Package the existing editor and embedded WASM; no server or internet in a DAW."""
from pathlib import Path
import base64, shutil, subprocess
root=Path(__file__).resolve().parent.parent
out=root/'plugin/resources/web'
out.mkdir(parents=True,exist_ok=True)
for name in ['style.css','plugin-bridge.js']:
    shutil.copyfile(root/'web'/name,out/name)
html=(root/'web/index.html').read_text().replace('<script src="/plugin-bridge.js"></script>','').replace('href="/style.css"','href="./style.css"').replace('<script type="module" src="/app.js"></script>','<script src="./engine-bytes.js"></script><script src="./plugin-bridge.js"></script><script defer src="./app.js"></script>')
(out/'index.html').write_text(html)
(out/'engine-bytes.js').write_text('globalThis.lxrWasmBase64="'+base64.b64encode((root/'web/lxr02.wasm').read_bytes()).decode()+'";\n')
subprocess.run([('npx.cmd' if __import__('os').name=='nt' else 'npx'),'--yes','esbuild@0.25.12',str(root/'web/app.js'),'--bundle','--format=iife','--target=safari14','--outfile='+str(out/'app.js')],check=True)
