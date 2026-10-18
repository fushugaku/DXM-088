#!/usr/bin/env python3
"""Package Windows VST3, CLAP and their offline editor with dependency notices."""
import hashlib
import json
import shutil
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'target/plugin-build-win/out'
RELEASE = ROOT / 'build/plugins/windows-x64'
RELEASE.mkdir(parents=True, exist_ok=True)
for name in ['DXM-088.vst3', 'DXM-088.clap', 'DXM-088.resources']:
    source, destination = OUT / name, RELEASE / name
    if not source.exists():
        raise RuntimeError(f'Missing build output: {source}')
    if destination.is_dir():
        shutil.rmtree(destination)
    if source.is_dir():
        shutil.copytree(source, destination)
    else:
        shutil.copy2(source, destination)
for name in ['README.md', 'SOURCES.json']:
    shutil.copy2(ROOT / 'plugin' / name, RELEASE / name)
shutil.copy2(ROOT / 'NOTICE.md', RELEASE / 'NOTICE.md')
shutil.copy2(ROOT / 'tools/install-windows.ps1', RELEASE / 'Install.ps1')
(RELEASE / 'Install.cmd').write_bytes(b'@echo off\r\npowershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Install.ps1"\r\npause\r\n')
examples = RELEASE / 'Examples'
examples.mkdir(exist_ok=True)
shutil.copy2(ROOT / 'examples/Polyrhythm Scenes.dxm088.json', examples)
notices = RELEASE / 'LICENSES'
notices.mkdir(exist_ok=True)
for name, path in [
    ('iPlug2', ROOT / '.deps/iplug2'),
    ('VST3', ROOT / '.deps/iplug2/Dependencies/IPlug/VST3_SDK'),
    ('CLAP', ROOT / '.deps/iplug2/Dependencies/IPlug/CLAP_SDK'),
    ('CLAP-helpers', ROOT / '.deps/iplug2/Dependencies/IPlug/CLAP_HELPERS'),
    ('WIL', ROOT / 'target/plugin-build-win/_deps/wil-src'),
    ('WebView2', ROOT / 'target/plugin-build-win/_deps/webview2'),
]:
    for path in path.glob('LICENSE*'):
        if path.is_file():
            shutil.copy2(path, notices / f'{name}-{path.name}')
wdl = (ROOT / '.deps/iplug2/WDL/heapbuf.h').read_text(encoding='utf-8')
(notices / 'WDL-heapbuf-notice.txt').write_text(wdl.split('*/', 1)[0] + '*/\n', encoding='utf-8')
webview = (ROOT / '.deps/iplug2/IPlug/Extras/WebView/IPlugWebView.h').read_text(encoding='utf-8')
(notices / 'iPlug2-WebView-MIT.txt').write_text(webview[webview.index('  MIT License'):webview.index('*/')].strip(' \n=') + '\n')
cargo = subprocess.check_output(['rustup', 'which', '--toolchain', 'stable', 'cargo'], text=True).strip()
metadata = json.loads(subprocess.check_output([cargo, 'metadata', '--locked', '--offline', '--format-version', '1', '--manifest-path', str(ROOT / 'plugin/engine/Cargo.toml')], text=True))
for package in metadata['packages']:
    if package.get('source'):
        for license_file in Path(package['manifest_path']).parent.glob('LICENSE*'):
            if license_file.is_file():
                shutil.copy2(license_file, notices / f'{package["name"]}-{license_file.name}')
# Validate actual PE files, including the expected x64 machine type.
for relative in ['DXM-088.clap', 'DXM-088.vst3/Contents/x86_64-win/DXM-088.vst3']:
    data = (RELEASE / relative).read_bytes()
    offset = struct.unpack_from('<I', data, 60)[0]
    if data[:2] != b'MZ' or data[offset:offset+4] != b'PE\0\0' or struct.unpack_from('<H', data, offset+4)[0] != 0x8664:
        raise RuntimeError(f'Not a Windows x64 PE module: {relative}')
for editor in ['DXM-088.resources/web', 'DXM-088.vst3/Contents/Resources/web']:
    for name in ['index.html', 'app.js', 'style.css', 'engine-bytes.js', 'plugin-bridge.js']:
        if not (RELEASE / editor / name).is_file():
            raise RuntimeError(f'Missing offline editor resource: {editor}/{name}')
manifest = {p.relative_to(RELEASE).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(RELEASE.rglob('*')) if p.is_file() and p.name != 'MANIFEST.json'}
(RELEASE / 'MANIFEST.json').write_text(json.dumps({'product': 'DXM-088', 'version': '0.5.0', 'platform': 'windows-x64', 'sha256': manifest}, indent=2) + '\n')
archive = ROOT / 'build/plugins/DXM-088-0.5.0-Windows-x64'
shutil.make_archive(str(archive), 'zip', root_dir=RELEASE.parent, base_dir=RELEASE.name)
zip_path = Path(str(archive) + '.zip')
zip_path.with_suffix('.zip.sha256').write_text(hashlib.sha256(zip_path.read_bytes()).hexdigest() + '  ' + zip_path.name + '\n')
print(f'Packaged and verified: {zip_path}')
