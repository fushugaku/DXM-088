# DXM-088

A digital drum synthesizer with a Rust audio engine, a polyrhythmic sequencer and
a shared browser / plug-in editor.

**macOS:** VST3, Audio Unit v2 and CLAP · **Windows x64:** VST3 and CLAP.

[Download releases](https://github.com/fushugaku/DXM-088/releases)
· [Plug-in manual](plugin/README.md)
· [Attribution](NOTICE.md)

![DXM-088 editor](docs/images/dxm088-desktop.png)

## Instrument

- Six synth voices and seven tracks, with closed/open hi-hat choke.
- FM, waveform oscillators, transient generators, drive and master FX.
- Three hit-triggered envelopes: Amp, Filter and Assignable.
- Two filters in serial or parallel: original LXR modes plus 17 Rustias filter models.
- 128 patterns in sixteen banks A–P; each track has its own step division, 1–64 step length and hit count.
- Straight, triplet and dotted divisions; microtiming, groove, global and per-track swing, and ratchets.
- Per-step probability, eight trigger conditions, Fill and two parameter locks.
- A sound/mix scene for every pattern, four macros, eight visible modulation slots with independent source/destination voices and two tempo LFOs.
- MIDI velocity, aftertouch and mod wheel; optional WAV layers on every track.
- Euclidean generation, protected sound mutation, undo and project autosave.
- WAV, MIDI and seven-stem ZIP export. macOS also supports native export drag into a DAW.
- Pattern-gate or direct drum MIDI modes, with seven configurable drum notes.
- Main stereo mix or seven individual stereo drum outputs in VST3, AU and CLAP.
- 394 host automation parameters (the original 292 IDs and ranges are preserved) and a local WebView editor with no server dependency.
- Save and update personal kits in the shared AU / VST3 / CLAP kit library, including sound, mix, FX and WAV layers.

**Save kit** stores a named kit directly in **KIT → My kits**. **Update kit** replaces
that saved kit; **Save as new** creates another. The native library is shared across
instances and formats in `~/Library/Application Support/DXM-088/Kits` on macOS or
`%APPDATA%\DXM-088\Kits` on Windows. DAW projects embed their sound and samples,
so they also open on machines without the saved kit library. Browser kits use IndexedDB.

**ENV** selects Amp / Filter / Assignable. **FILTER** selects Filter 1 / Filter 2 / Drive.
Filter 1 keeps its original **LXR type** when Model is LXR; the Model control adds
Rustias Ladder, Diode, K35, SEM, Morph and SVF families. Filter 2 has its own type,
cutoff, resonance, drive and envelope depth. Serial chains both filters; Parallel
blends their outputs using Filter balance. The added models default to bypass.

**Timing → Track swing** adds 0–50% to global swing, capped at 50% of a step.
It is stored with each track rhythm. **Matrix** exposes eight ready-to-edit slots.
Choose a macro, MIDI source, LFO, envelope or Voice parameter; voice-based sources
have independent From voice / Source parameter controls. To voice and Destination
select the recipient, defaulting to the current voice. Modulation changes the live
sound without overwriting saved controls. Closed/Open Hat share one synthesis
voice; their envelope sources follow the last triggered hat. Active legacy routes
9–16 remain available when loading older projects.

Held patterns do not replay the first hit when DAW PPQ wraps partway through an
audio block. In-flight ratchets survive the wrap; a new note keeps normal startup.

In Patterns mode, edits made while holding a MIDI note remain in its pattern.
Each note retains its own rhythm: steps, track lengths, divisions, timing and locks.
The kit sound, mixer and FX are shared across patterns. Replaying a note and opening
the saved DAW project restore its rhythm without reverting the common sound.
The separate Scenes controls remain available for explicitly captured sound scenes;
editing under a held MIDI note never creates scenes automatically.

In **MIDI → Patterns**, every MIDI note **0–127 (C−2–G8)** selects its own pattern and plays while held.
New projects map **0 → A01**, **7 → A08**, **8 → B01**, **127 → P08**.
The last held note wins; releasing it returns to the previous held note.
Releasing all notes stops the sequencer. Pattern changes can be immediate,
on the next 1/16, or on the next 4/4 bar. The sequencer follows the host tempo,
PPQ, seeking and loops. Octave names vary by DAW; MIDI numbers are unambiguous.
The bank selector shows eight slots at a time and follows incoming MIDI notes.
The **A01** field rotates the mapping while keeping all 128 notes available.
**Play / Space** previews the selected pattern without a MIDI clip. A held MIDI
note takes over; releasing the last note stops the sequencer. Keyboard shortcuts
A S D F G H J audition individual voices when the editor has focus. VST3 accepts
all 16 MIDI channels.

**MIDI → Drums** plays one percussion hit per note, including velocity and hi-hat
choke. Note-off leaves the drum's natural tail. The editable defaults are
**36 / 45 / 47 / 38 / 49 / 42 / 46** for Drum 1, Drum 2, Drum 3, Snare, Cymbal,
Closed Hat and Open Hat. MIDI mode, routing and note assignments are saved per instance.

**Output → Separate** sends those seven tracks to named stereo buses, after voice
processing, levels, pan and master gain, before shared master FX. Mix is silent in
this mode. **Output → Mix** retains the original stereo mix and shared FX, with
silent auxiliary buses. In Ableton, create audio tracks, select the DXM-088 track
in **Audio From**, choose a drum output in the lower chooser and set **Monitor: In**.
See [Ableton's multi-out guide](https://help.ableton.com/hc/en-us/articles/209773065-Using-multi-out-plug-ins).

DXM-088 is the new name of the earlier local LXR02 Rust instrument. Existing
format IDs, parameter IDs and project data remain compatible. Avoid installing
both names of the same plug-in format together: they identify the same instrument.
Projects use JSON v4 and can import earlier v1/v2/v3 files, including `.lxr02.json`.
Older projects keep their original MIDI origin (usually 24), eight patterns and
scenes; the added slots receive independent copies of those eight templates.
The demo is [Polyrhythm Scenes](examples/Polyrhythm%20Scenes.dxm088.json).

## Install

Download a platform archive from [Releases](https://github.com/fushugaku/DXM-088/releases).

**macOS 11+, Intel and Apple Silicon:** open the `.pkg` and choose VST3 / AU / CLAP.
It installs for the current user in `~/Library/Audio/Plug-Ins/`.
The bundles use ad-hoc signing; the installer has no Developer ID notarization.

**Windows 10/11 x64:** extract the ZIP and run `Install.cmd`. It copies VST3 and
CLAP into the current user's standard plug-in directories, without admin rights.
Keep `DXM-088.resources` beside `DXM-088.clap` when installing manually.
The VST3 bundle contains its editor resources. Install the
[Microsoft WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
if it is missing, then rescan plug-ins in your DAW.
AU is available on macOS only. Windows ARM64, 32-bit Windows and Linux plug-in
adapters are not provided. A successful build does not establish compatibility
with every DAW.

## Build

Dependencies: Rust stable, Python 3, Node.js/npm and CMake 3.22+. Native plug-ins
also require Xcode command line tools on macOS, or Visual Studio 2022 C++ Build
Tools with the Windows SDK on Windows. SDK revisions are pinned in
[`plugin/SOURCES.json`](plugin/SOURCES.json).

Browser instrument:

```sh
rustup target add wasm32-unknown-unknown
./run.sh
# Open http://127.0.0.1:8732
```

macOS universal plug-ins and installer:

```sh
./tools/build-plugins.sh
```

Windows x64, from PowerShell:

```powershell
./tools/build-plugins.ps1
```

Outputs go to `build/plugins/`. The Windows Actions workflow builds on a Windows
runner, checks the Rust engine and legacy project migration, loads the compiled
CLAP module and packages offline editor resources and dependency licenses.
Pushing a `v*` tag publishes the Windows ZIP and SHA-256 file to a GitHub release.

```sh
cargo test --locked
cargo test --locked --manifest-path plugin/engine/Cargo.toml
node tools/verify-project-format.mjs
node tools/verify-pattern-bank.mjs
# After building WASM:
node tools/verify-performance.mjs
```

The Rust crates, exported C/WASM function names and some source filenames retain
`lxr02` internally for compatibility. The command-line executable is `dxm088`:

```sh
target/release/dxm088 render demo.wav --bars 4 --bpm 128 --kit 0
```

## DSP provenance

The synthesis core was translated from LXR-02 firmware 1.71: phase-modulation
oscillators, envelopes, SVF/LP2 filters, original transient and waveform tables,
drive, tube/fold/clip, compressor, ring modulation and delay. Firmware-derived
fixtures record ARM reference outputs for the DSP tests. The complete firmware,
local decompilation workspace, private DAW projects and local QA artifacts are
excluded from this repository.

DXM-088 is an independent project and is not affiliated with Erica Synths or
Sonic Potions. Original firmware and ROM rights remain with their owners.
See [NOTICE.md](NOTICE.md) for provenance and third-party component notices.
