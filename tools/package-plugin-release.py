#!/usr/bin/env python3
"""Add source/SDK attribution, usage notes and hashes to a locally built release."""
import hashlib,json,shutil,subprocess
from pathlib import Path
root=Path(__file__).resolve().parent.parent
release=root/'build/plugins/dxm088-macos-universal'
shutil.copy2(root/'plugin/README.md',release/'README.md')
shutil.copy2(root/'plugin/SOURCES.json',release/'SOURCES.json')
shutil.copy2(root/'NOTICE.md',release/'NOTICE.md')
examples=release/'Examples';examples.mkdir(exist_ok=True)
shutil.copy2(root/'examples/Polyrhythm Scenes.dxm088.json',examples/'Polyrhythm Scenes.dxm088.json')
notices=release/'LICENSES';notices.mkdir(exist_ok=True)
for name,path in [('iPlug2',root/'.deps/iplug2'),('VST3',root/'.deps/iplug2/Dependencies/IPlug/VST3_SDK'),('CLAP',root/'.deps/iplug2/Dependencies/IPlug/CLAP_SDK'),('CLAP-helpers',root/'.deps/iplug2/Dependencies/IPlug/CLAP_HELPERS')]:
 for p in path.glob('LICENSE*'):
  if p.is_file():shutil.copy2(p,notices/(name+'-'+p.name))
# Include licenses from statically linked Rust crates and the WebView/WDL layers.
webview=root/'.deps/iplug2/IPlug/Extras/WebView/IPlugWebView.h'
if webview.is_file():
 text=webview.read_text();notice=text[text.index('  MIT License'):text.index('*/')].strip(' \n=')
 (notices/'iPlug2-WebView-MIT.txt').write_text(notice+'\n')
wdl=(root/'.deps/iplug2/WDL/heapbuf.h').read_text(encoding='utf-8')
(notices/'WDL-heapbuf-notice.txt').write_text(wdl.split('*/',1)[0]+'*/\n',encoding='utf-8')
cargo=subprocess.check_output(['rustup','which','cargo'],text=True).strip()
metadata=json.loads(subprocess.check_output([cargo,'metadata','--locked','--format-version','1','--manifest-path',str(root/'plugin/engine/Cargo.toml')],text=True))
for package in metadata['packages']:
 if package.get('source'):
  folder=Path(package['manifest_path']).parent
  for license_file in folder.glob('LICENSE*'):
   if license_file.is_file():shutil.copy2(license_file,notices/(package['name']+'-'+license_file.name))
manifest={}
for extension in ['vst3','component','clap']:
 binary=release/f'DXM-088.{extension}/Contents/MacOS/DXM-088'
 manifest[f'DXM-088.{extension}']={'sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'architectures':subprocess.check_output(['lipo','-archs',str(binary)],text=True).strip().split()}
(release/'MANIFEST.json').write_text(json.dumps(manifest,indent=2)+'\n')
subprocess.run(['ditto','-c','-k','--norsrc','--noextattr','--keepParent',str(release),str(root/'build/plugins/DXM-088-macOS-universal.zip')],check=True)
