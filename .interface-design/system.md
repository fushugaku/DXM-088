# LXR / digital percussion instrument

Intent: a playable drum instrument for a musician. Immediate sound, fast editing,
one visible pattern, no onboarding or account UI. Desktop studio and touch phone.

Domain: drum voices, channel strips, step lights, oscilloscope, patch memory,
matte metal faceplate, transport. The signature is a seven-track amber step matrix
aligned to seven mixer strips and a compact synthesizer editor.

Palette: graphite #141515, faceplate #1b1c1c, inset #101212, warm white #ebe7df,
secondary #aaa99f, tertiary #7d807b, muted #60645e, amber #eab460,
oxide #cf7766, green #a2ba94. Borders: #252827, #363a36, #60675e,
focus #eab460. Controls have #212422 background, #42473f border.

Depth: borders and subtle surface changes. Two inset display areas. No colored
glows, decorative gradient, rounded dashboard cards or sidebar. Use a long
instrument panel with strong alignment, seven channel strips and dense sequencer.

Typography: system sans for readable labels, system monospace for parameter values,
tempo, pattern numbers and technical marks. Display title wide and light. Four
levels of text contrast. Uppercase compact labels only where needed on controls.

Spacing: 4px base; 4/8/12/16/24/32/48. Radius 2px controls, 4px panels.
All toolbar controls 36px tall. Mobile targets at least 44px where practical.

Interaction: amber is selected/on/running. White is hover. Visible keyboard focus.
Knobs support vertical pointer drag, wheel, arrows, Home/End, double-click reset.
Knob motion is relative: 320px for the full range, Shift for 10× finer control.
Host parameter updates preserve the control node, pointer capture and focus.
Steps support click and keyboard; right click/long press opens step properties.
Space is transport; A S D F G H J trigger the seven tracks.

Responsive: instruments reflow, controls retain equal height, sequencer scrolls
within its own surface at narrow widths, seven mixer strips remain aligned.
No document overflow at 390px. Motion respects reduced-motion preferences.

Dropdowns: fixed position in the active dialog (otherwise the document body),
8px viewport inset, 4px gap to the trigger, maximum height 280px, minimum width
208px. Open upward when the space below is insufficient. Scroll within the list;
never extend a dialog's scroll area. Selected options scroll into view. Arrow
keys, Home/End, type-ahead, Escape and Tab work without scrolling the page.
Rows are 36px on desktop, 44px on touch-size layouts. Reuse the panel palette.


Performance extension (0.4): one tabbed working surface below the sequencer, sharing
its selected track. STEP / LENGTH / HITS live in the sequencer footer. A track label
shows its division and length. Timing, Perform, Scenes, Matrix, Sample, Mutate and
Export use the existing panel/control palette. Align label and input heights; 36px
controls on desktop, 44px in mobile feature panels. Do not repeat explanatory copy.
The step editor is a right-hand drawer, full width on a phone, with grouped search
for lock parameters. Only the parameter list scrolls while open. Keep project audio
assets outside undo JSON copies so turning a knob never duplicates WAV data.

Pattern bank (0.6): 128 independent slots in sixteen banks A–P, eight buttons
per bank. Replace the PATTERN caption with a compact 56px bank selector, matching
the button row height. Selecting a bank retains the slot; incoming MIDI follows
the active bank. Scene buttons show the same eight slots as the top row. MIDI
names use C−2 = 0 through G8 = 127. Keep sample PCM and unedited bank slots out
of routine undo snapshots.

Native transport (0.6.1): Play / Space previews a pattern without an external
MIDI note. MIDI takes priority, disabling the preview button while a note is held.
Release of the last held note stops playback; saved projects never start preview
automatically. Keep focused Length/Hits fields stable during grid updates and
commit their displayed bounds on blur or Enter. Native WebView must pass keyboard
events through WebKit/Cocoa instead of claiming unhandled keys.

Native routing (0.7): a compact strip between transport and voices uses two
segmented controls: MIDI Patterns / Drums and Output Mix / Separate. Amber marks
the selected segment; both controls have equal height. Patterns shows the A01
origin; Drums shows seven editable note assignments with note names and ± controls.
Keep note fields stable during host updates. On phones the switches stack and
note assignments form three columns with 44px controls. A single conditional
line explains that Separate bypasses master FX and silences Mix, avoiding doubled
audio when the seven named stereo buses feed DAW tracks.
