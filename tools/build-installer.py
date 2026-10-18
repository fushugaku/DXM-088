#!/usr/bin/env python3
"""Build and verify a macOS Installer product from the universal plug-in release."""
import hashlib
import html
import json
import plistlib
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RELEASE = ROOT / 'build/plugins/dxm088-macos-universal'
FORMATS = [('vst3', 'VST3', 'VST3'), ('component', 'Components', 'Audio Unit v2'),
           ('clap', 'CLAP', 'CLAP')]
PREFIX = 'io.github.fushugaku.DXM088.pkg'


def run(*args):
    return subprocess.check_output([str(arg) for arg in args], text=True).strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def page(title, body):
    return ('<!doctype html><html lang="ru"><head><meta charset="utf-8">'
            '<style>body{font:14px -apple-system,BlinkMacSystemFont,sans-serif;'
            'line-height:1.5;margin:24px;color:#252525}h1{font-size:26px;'
            'font-weight:600;margin:0 0 12px}p{margin:12px 0}code{font-size:12px}'
            '</style></head><body><h1>' + html.escape(title) + '</h1>' + body +
            '</body></html>')


def verify_payload(source, extracted):
    """Compare every shipped file, symlink and executable bit after pkg expansion."""
    expected = {p.relative_to(source) for p in source.rglob('*')}
    actual = {p.relative_to(extracted) for p in extracted.rglob('*')}
    if expected != actual:
        raise RuntimeError(f'Payload paths differ: {source}')
    for relative in expected:
        src, dst = source / relative, extracted / relative
        if src.is_symlink():
            if not dst.is_symlink() or src.readlink() != dst.readlink():
                raise RuntimeError(f'Payload symlink differs: {relative}')
        elif src.is_file():
            if digest(src) != digest(dst) or (src.stat().st_mode & 0o111) != (dst.stat().st_mode & 0o111):
                raise RuntimeError(f'Payload file differs: {relative}')


def main():
    if sys.platform != 'darwin':
        raise SystemExit('The .pkg installer must be built on macOS.')
    versions = set()
    binaries = {}
    manifest = json.loads((RELEASE / 'MANIFEST.json').read_text())
    for suffix, _, _ in FORMATS:
        bundle = RELEASE / f'DXM-088.{suffix}'
        info = plistlib.loads((bundle / 'Contents/Info.plist').read_bytes())
        versions.add(info['CFBundleShortVersionString'])
        binary = bundle / 'Contents/MacOS/DXM-088'
        architectures = run('lipo', '-archs', binary).split()
        if set(architectures) != {'arm64', 'x86_64'}:
            raise RuntimeError(f'Not a universal release: {bundle}')
        if digest(binary) != manifest[bundle.name]['sha256']:
            raise RuntimeError(f'Release manifest mismatch: {bundle}')
        run('codesign', '--verify', '--strict', bundle)
        binaries[bundle.name] = {'sha256': digest(binary), 'architectures': architectures}
    if len(versions) != 1:
        raise RuntimeError(f'Bundle versions differ: {versions}')
    version = versions.pop()
    destination = ROOT / f'build/plugins/DXM-088-{version}-macOS-universal.pkg'
    report = {'version': version, 'installer': str(destination),
              'domain': 'CurrentUserHomeDirectory', 'minimumMacOS': '11.0',
              'signing': 'Unsigned installer; ad-hoc signed plug-in bundles',
              'binaries': binaries}
    with tempfile.TemporaryDirectory(prefix='lxr02-installer-') as temporary:
        work = Path(temporary)
        packages = work / 'packages'
        resources = work / 'resources'
        packages.mkdir()
        resources.mkdir()
        (resources / 'welcome.html').write_text(page('DXM-088',
            f'<p>{html.escape(version)} · VST3 · AU · CLAP<br>macOS 11+ · Intel и Apple Silicon</p>'
            '<p>Выберите форматы на следующем экране. Плагины устанавливаются для текущего '
            'пользователя в <code>~/Library/Audio/Plug-Ins</code>.</p>'
            '<p>Перед установкой закройте экземпляры DXM-088 в DAW.</p>'))
        (resources / 'conclusion.html').write_text(page('DXM-088 установлен',
            '<p>Откройте DAW и обновите список плагинов. Добавьте <b>DXM-088</b> '
            'на инструментальную MIDI-дорожку.</p><p>Удерживайте MIDI-ноты <b>24–31</b>: '
            'они запускают паттерны <b>A01–A08</b>. После отпускания всех нот секвенсор '
            'останавливается.</p><p>Документация и лицензии: '
            '<code>~/Library/Application Support/DXM-088</code>.</p>'))
        distribution = ET.Element('installer-gui-script', {'minSpecVersion': '2'})
        ET.SubElement(distribution, 'title').text = f'DXM-088 {version}'
        ET.SubElement(distribution, 'options', {'customize': 'always',
            'require-scripts': 'false', 'hostArchitectures': 'arm64,x86_64'})
        ET.SubElement(distribution, 'domains', {'enable_anywhere': 'false',
            'enable_currentUserHome': 'true', 'enable_localSystem': 'false'})
        check = ET.SubElement(distribution, 'installation-check')
        os_versions = ET.SubElement(check, 'allowed-os-versions')
        ET.SubElement(os_versions, 'os-version', {'min': '11.0'})
        for name in ['welcome', 'conclusion']:
            ET.SubElement(distribution, name, {'file': f'{name}.html', 'mime-type': 'text/html'})
        outline = ET.SubElement(distribution, 'choices-outline')
        payloads = {}
        locations = {}
        for suffix, folder, title in FORMATS + [('docs', '', '')]:
            payload = work / f'payload-{suffix}'
            payload.mkdir()
            if suffix == 'docs':
                documents = payload / 'Library/Application Support/DXM-088'
                documents.mkdir(parents=True)
                for filename in ['README.md', 'NOTICE.md', 'SOURCES.json', 'MANIFEST.json', 'LICENSES', 'Examples']:
                    run('ditto', RELEASE / filename, documents / filename)
                description = 'Документация и лицензии'
                title = description
            else:
                relative = Path(f'Library/Audio/Plug-Ins/{folder}/DXM-088.{suffix}')
                bundle = payload / relative
                bundle.parent.mkdir(parents=True)
                run('ditto', RELEASE / bundle.name, bundle)
                locations[title] = f'~/{relative}'
                description = f'~/{relative}'
            identifier = f'{PREFIX}.{suffix}'
            component = work / f'{suffix}.plist'
            if suffix == 'docs':
                components = []
            else:
                components = [{'RootRelativeBundlePath': str(relative),
                    'BundleIsRelocatable': False, 'BundleIsVersionChecked': True,
                    'BundleHasStrictIdentifier': True, 'BundleOverwriteAction': 'upgrade'}]
            component.write_bytes(plistlib.dumps(components))
            run('pkgbuild', '--root', payload, '--component-plist', component,
                '--identifier', identifier, '--version', version, '--install-location', '/',
                '--ownership', 'recommended', '--compression', 'legacy', packages / f'{suffix}.pkg')
            ET.SubElement(outline, 'line', {'choice': suffix})
            choice = ET.SubElement(distribution, 'choice', {'id': suffix, 'title': title,
                'description': description, 'start_selected': 'true',
                'visible': 'false' if suffix == 'docs' else 'true'})
            ET.SubElement(choice, 'pkg-ref', {'id': identifier})
            ET.SubElement(distribution, 'pkg-ref', {'id': identifier,
                'version': version, 'onConclusion': 'None'}).text = f'{suffix}.pkg'
            payloads[suffix] = payload
        distribution_file = work / 'Distribution.xml'
        ET.indent(distribution)
        ET.ElementTree(distribution).write(distribution_file, encoding='utf-8', xml_declaration=True)
        run('productbuild', '--distribution', distribution_file, '--package-path', packages,
            '--resources', resources, '--identifier', PREFIX, '--version', version, destination)
        expanded = work / 'expanded'
        run('pkgutil', '--expand-full', destination, expanded)
        for suffix, payload in payloads.items():
            verify_payload(payload, expanded / f'{suffix}.pkg/Payload')
        domain = run('installer', '-dominfo', '-pkg', destination)
        if 'CurrentUserHomeDirectory' not in domain or 'LocalSystem' in domain:
            raise RuntimeError(f'Unexpected installation domains: {domain}')
        choices = plistlib.loads(run('installer', '-showChoiceChangesXML', '-pkg', destination,
                                    '-target', 'CurrentUserHomeDirectory').encode())
        selected = {c['choiceIdentifier']: c['attributeSetting'] for c in choices
                    if c['choiceAttribute'] == 'selected'}
        if not all(selected.get(suffix) == 1 for suffix in payloads):
            raise RuntimeError(f'Default choices are not selected: {selected}')
        changes = work / 'choices.plist'
        changes.write_bytes(plistlib.dumps([{'choiceIdentifier': 'clap',
            'choiceAttribute': 'selected', 'attributeSetting': 0}]))
        changed = plistlib.loads(run('installer', '-showChoicesAfterApplyingChangesXML', changes,
            '-pkg', destination, '-target', 'CurrentUserHomeDirectory').encode())
        changed_selected = {c['choiceIdentifier']: c['attributeSetting'] for c in changed
                            if c['choiceAttribute'] == 'selected'}
        if changed_selected.get('clap') != 0 or changed_selected.get('vst3') != 1 or changed_selected.get('component') != 1:
            raise RuntimeError(f'Format selection failed: {changed_selected}')
        report['checks'] = {'expandedPayloadMatches': True, 'codeSignaturesValid': True,
            'universalArchitectures': True, 'domains': domain, 'defaultChoices': selected,
            'independentFormatSelection': True}
        report['locations'] = locations
    report['sha256'] = digest(destination)
    report['bytes'] = destination.stat().st_size
    (ROOT / 'artifacts').mkdir(exist_ok=True)
    (ROOT / 'artifacts/installer-verification.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'Built and verified: {destination}\nSHA-256: {report["sha256"]}')


if __name__ == '__main__':
    main()
