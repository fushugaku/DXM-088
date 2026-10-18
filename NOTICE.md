# DXM-088 — attribution

DXM-088 is an independent digital drum instrument. It is not affiliated with,
endorsed by, or an official product of Erica Synths or Sonic Potions. LXR-02 is
mentioned only to identify the original firmware and DSP reference.

The DSP in `src/firmware/` was translated from LXR-02 firmware 1.71. The waveform,
transient and lookup data in `src/data/`, and firmware-derived test fixtures in
`tests/fixtures/`, retain the rights of their original owners. Publishing this
repository does not grant additional rights to that third-party material.
The complete firmware and the local decompilation workspace are not included.

Additional nonlinear filters in `src/rustias_filter.rs` are ported from
[fushugaku/Rustias](https://github.com/fushugaku/Rustias),
`crates/radias-synth-infrastructure/src/circuit/filters.rs`, commit
`40b1a175f8212b829ce8a1386caedcb8102cd3f5`. The port makes the sample rate
explicit for the DXM-088 DSP clock. These are independent Rustias designs.

The host adapters depend on iPlug2, WDL, Steinberg VST3 SDK, CLAP and CLAP helpers.
Windows builds also use Microsoft WIL and WebView2. SDK source locations and
pinned revisions are in `plugin/SOURCES.json`; binary releases include dependency
license notices in `LICENSES/`. Rust dependency revisions are in the lockfile.

Legacy `LxR2` / `ErRs` format identifiers, `local.ErikaRust` bundle identities,
internal crate names and the `lxr02-web` JSON format are retained so saved
projects and automation remain compatible after the rename to DXM-088.

No project-wide open-source license is granted by this notice. The licenses of
individual third-party components continue to apply to those components.
