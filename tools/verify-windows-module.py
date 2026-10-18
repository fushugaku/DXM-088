#!/usr/bin/env python3
"""Load the compiled CLAP on Windows and inspect its real plugin descriptor."""
import ctypes as C
import json
from pathlib import Path

class Version(C.Structure):
    _fields_ = [('major', C.c_uint32), ('minor', C.c_uint32), ('revision', C.c_uint32)]

class Descriptor(C.Structure):
    _fields_ = [('version', Version)] + [(key, C.c_char_p) for key in
        ['id', 'name', 'vendor', 'url', 'manual_url', 'support_url', 'version_string', 'description']]

Init = C.CFUNCTYPE(C.c_bool, C.c_char_p)
Deinit = C.CFUNCTYPE(None)
GetFactory = C.CFUNCTYPE(C.c_void_p, C.c_char_p)
class Entry(C.Structure):
    _fields_ = [('version', Version), ('init', Init), ('deinit', Deinit), ('get_factory', GetFactory)]

Count = C.CFUNCTYPE(C.c_uint32, C.c_void_p)
GetDescriptor = C.CFUNCTYPE(C.POINTER(Descriptor), C.c_void_p, C.c_uint32)
class Factory(C.Structure):
    _fields_ = [('count', Count), ('descriptor', GetDescriptor), ('create', C.c_void_p)]

path = Path(__file__).resolve().parent.parent / 'build/plugins/windows-x64/DXM-088.clap'
module = C.CDLL(str(path))
entry = Entry.in_dll(module, 'clap_entry')
assert entry.init(str(path).encode())
try:
    address = entry.get_factory(b'clap.plugin-factory')
    assert address
    factory = C.cast(address, C.POINTER(Factory)).contents
    assert factory.count(address) == 1
    descriptor = factory.descriptor(address, 0).contents
    assert descriptor.name == b'DXM-088', descriptor.name
    assert descriptor.id == b'local.ErikaRust.LXR02', descriptor.id
    version = json.loads((path.parent / 'MANIFEST.json').read_text())['version']
    assert descriptor.version_string == version.encode(), descriptor.version_string
    print(f'Windows x64 CLAP loaded: DXM-088 {version}; legacy session identity preserved.')
finally:
    entry.deinit()
