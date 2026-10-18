import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {migrateProject} from '../web/project-format.js';
import {PATTERN_COUNT,patternLabel,patternNote,noteName} from '../web/pattern-bank.js';
import {captureHistory,restoreHistory} from '../web/project-history.js';
import {featureCommands} from '../web/feature-state.js';

const fixture=JSON.parse(await readFile(new URL('../tests/fixtures/legacy-session.lxr02.json',import.meta.url)));
const s=migrateProject(fixture);
assert.equal(s.version,4);
assert.equal(s.patterns.length,PATTERN_COUNT);
assert.equal(s.scenes.length,PATTERN_COUNT);
assert.equal(s.plugin.baseNote,24);
assert.deepEqual(s.patterns[127],s.patterns[7]);
s.patterns[127].tracks[6][63].v=.123;
assert.notEqual(s.patterns[7].tracks[6][63].v,.123,'copied patterns must be independent');
assert.equal(patternLabel(0),'A01');assert.equal(patternLabel(7),'A08');
assert.equal(patternLabel(8),'B01');assert.equal(patternLabel(127),'P08');
assert.equal(noteName(0),'C-2');assert.equal(noteName(127),'G8');
for(const base of [0,24,60,120]){
 const notes=Array.from({length:128},(_,p)=>patternNote(p,base));
 assert.equal(new Set(notes).size,128);
 assert(notes.every(n=>n>=0&&n<=127));
}

s.pattern=127;
const one=captureHistory(s),all=captureHistory(s,true);
assert(one.json.length<all.json.length/50,'routine edits must not copy the entire bank');
s.patterns[127].lengths[0]=13;s.pattern=0;
const restored=restoreHistory(s,one);
assert.equal(restored.pattern,127);assert.equal(restored.patterns[127].lengths[0],16);
assert.equal(restored.patterns[0],s.patterns[0],'untouched slots must retain their identity');
const copy=captureHistory(restored,[0]);
restored.patterns[0]=structuredClone(restored.patterns[127]);
const undone=restoreHistory(restored,copy);
assert.deepEqual(undone.patterns[0],s.patterns[0]);
const sample='A'.repeat(200000);
undone.samples[0]={name:'Layer.wav',data:sample};
const withSample=captureHistory(undone);
assert(!withSample.json.includes(sample));
assert.equal(restoreHistory(undone,withSample).samples[0].data,sample);
const partial=featureCommands(s,[127]);
assert(partial.filter(([fn])=>fn==='lxr_track_settings').every(([,args])=>args[0]===127));
assert.deepEqual(partial.filter(([fn])=>fn==='lxr_scene_enable').map(([,args])=>args[0]),[127]);

const module=await WebAssembly.compile(await readFile(new URL('../web/lxr02.wasm',import.meta.url)));
const e=new WebAssembly.Instance(module,{}).exports;e.lxr_init(44100);
assert.equal(e.lxr_pattern_count(),128);
for(let p=0;p<128;p++){
 e.lxr_pattern(p);e.lxr_length(p,0,p%64+1);
 e.lxr_set_step(p,0,63,.5,1,p%24,1);
 assert.equal(e.lxr_get_length(p,0),p%64+1);
 assert.equal(e.lxr_get_step(p,0,63,0),.5);
}
e.lxr_pattern(127);e.lxr_length(127,0,1);e.lxr_set_step(127,0,0,1,1,0,1);
e.lxr_play();e.lxr_process(512);
assert(new Float32Array(e.memory.buffer,e.lxr_buffer(),1024).some(x=>Math.abs(x)>.001));
console.log('128 MIDI patterns: labels, wrap mapping, legacy migration, independent edits, compact undo and upper-bank WASM audio passed.');
