#!/usr/bin/env node
import assert from 'node:assert/strict';
import './verify-expanded.mjs';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {migrateProject} from '../web/project-format.js';
import {featureCommands,validateFeatures,uploadSamples,pcmToBase64,pcmFromBase64,divisions} from '../web/feature-state.js';
import {bindDefinitions,defs} from '../web/parameters.js';
import {renderPattern,wavBytes,midiBytes,zipBytes,readWav} from '../web/export-engine.js';
const module=await WebAssembly.compile(await readFile(new URL('../web/lxr02.wasm',import.meta.url))),e=new WebAssembly.Instance(module,{}).exports;e.lxr_init(44100);bindDefinitions(e);
const s=migrateProject(JSON.parse(await readFile(new URL('../tests/fixtures/legacy-session.lxr02.json',import.meta.url))));
const number=(x,min,max)=>{assert(Number.isFinite(x)&&x>=min-.0001&&x<=max+.0001);return Math.max(min,Math.min(max,x));};
validateFeatures(s,structuredClone(s),number,defs);
s.bpm=120;s.pattern=0;s.morph=0;
for(const p of s.patterns)for(const row of p.tracks)for(const step of row)Object.assign(step,{v:0,prob:1,note:0,ratchet:1,locks:[[255,0],[255,0]]});
for(let t=0;t<3;t++){s.patterns[0].lengths[t]=3;for(let k=0;k<3;k++)s.patterns[0].tracks[t][k].v=1;}
s.patterns[0].divisions[0]=4;s.patterns[0].divisions[1]=3;s.patterns[0].divisions[2]=9;
const commands=[['lxr_panic',[]],['lxr_tempo',[s.bpm,0]],['lxr_pattern',[0]]];
for(let p=0;p<s.patterns.length;p++)for(let t=0;t<7;t++){commands.push(['lxr_length',[p,t,s.patterns[p].lengths[t]]]);for(let k=0;k<64;k++){const a=s.patterns[p].tracks[t][k];commands.push(['lxr_set_step',[p,t,k,a.v,a.prob,a.note,a.ratchet]]);}}
commands.push(...featureCommands(s));
const result=await renderPattern(module,commands,s,{beats:1,audio:true});
assert.deepEqual(result.hits.filter(h=>h.track===0).map(h=>h.frame),[0,5513,11025,16538]);
assert.deepEqual(result.hits.filter(h=>h.track===1).map(h=>h.frame),[0,11025]);
assert.deepEqual(result.hits.filter(h=>h.track===2).map(h=>h.frame),[0,3675,7350,11025,14700,18375]);
const wav=wavBytes(result.pcm,44100);assert.equal(new DataView(wav.buffer).getUint32(40,true),result.pcm.length*2);assert(result.pcm.every(Number.isFinite));
const midi=midiBytes(result.hits,120,1);assert.equal(new TextDecoder().decode(midi.slice(0,4)),'MThd');assert.equal(new DataView(midi.buffer).getUint16(10),8);
// Exported percussion must address the selected drums, even on pitched steps.
const mapped=midiBytes([{track:3,frame:0,note:7,velocity:1}],120,1,{drumNotes:[36,45,47,127,49,42,46]});
assert([...mapped].some((x,i)=>x===153&&mapped[i+1]===127&&mapped[i+2]===127));
const gate=midiBytes([],120,4,{gateNote:26});assert.equal(new DataView(gate.buffer).getUint16(10),2);assert([...gate].some((x,i)=>x===144&&gate[i+1]===26));
const fixture=readWav((await readFile(new URL('../tests/fixtures/performance-sample.wav',import.meta.url))).buffer.slice(0));
assert.equal(fixture.rate,44100);assert.equal(fixture.pcm.length,11025);
const data=pcmToBase64(fixture.pcm),decoded=pcmFromBase64(data);assert.equal(decoded.length,11025);assert(decoded.some(x=>Math.abs(x)>.2));
const sample={name:'Own sample.wav',rate:44100,data,gain:.7,tune:0,start:0,end:1,decay:4,replace:true};uploadSamples(e,[sample,null,null,null,null,null,null]);e.lxr_sample_settings(0,.7,0,0,1,4,1);e.lxr_trigger(0,1,0);e.lxr_process(512);assert(new Float32Array(e.memory.buffer,e.lxr_buffer(),1024).some(x=>Math.abs(x)>.01));
const zip=zipBytes([{name:'mix.wav',bytes:wav},{name:'notes.mid',bytes:midi}]);
await mkdir(new URL('../artifacts/',import.meta.url),{recursive:true});await writeFile(new URL('../artifacts/performance-export.zip',import.meta.url),zip);
const report={divisions:divisions.length,independentTracks:true,tripletHits:6,midiTracks:8,gateNote:26,sampleFrames:11025,wavFrames:result.pcm.length/2,allAudioFinite:true,zipFiles:2};
await writeFile(new URL('../artifacts/performance-wasm-0.4.0.json',import.meta.url),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report));
