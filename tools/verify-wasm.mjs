#!/usr/bin/env node
// Real WASM engine verification without browser mocks or JS synthesis.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {performance} from 'node:perf_hooks';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const bytes=await readFile(path.join(root,'web/lxr02.wasm'));
const module=await WebAssembly.compile(bytes);
const e=new WebAssembly.Instance(module,{}).exports;
e.lxr_init(44100);
assert.equal(e.lxr_get_param(0,0),49);
assert.equal(e.lxr_get_length(0,0),16);
assert.equal(e.lxr_get_step(0,0,0,3),1);
let memory=e.memory.buffer;
const energy=(count)=>{e.lxr_process(count);const data=new Float32Array(e.memory.buffer,e.lxr_buffer(),count*2);let sum=0;for(const x of data){assert(Number.isFinite(x)&&Math.abs(x)<=1.001);sum+=x*x;}return sum;};
assert.equal(energy(4096),0);
const voiceEnergy=[];
for(let t=0;t<7;t++){e.lxr_init(44100);e.lxr_trigger(t,1,0);const sum=energy(4096);assert(sum>0.01);voiceEnergy.push(sum);}
const fxEnergy=[];
for(const sr of [44100,48000,96000])for(let kind=0;kind<8;kind++){
 e.lxr_init(sr);e.lxr_fx(0,kind);e.lxr_fx(1,1);e.lxr_fx(2,0.03);e.lxr_fx(7,0.23);e.lxr_play();
 const stable=e.memory.buffer;let sum=0;for(let b=0;b<4;b++){sum+=energy(4096);assert.equal(e.memory.buffer,stable,'FX render grew WASM memory');}
 assert(sum>0.001,`silent effect ${kind} at ${sr}Hz`);fxEnergy.push({sampleRate:sr,kind,energy:Number(sum.toFixed(5))});
}
e.lxr_init(44100);e.lxr_tempo(128,0);e.lxr_play();memory=e.memory.buffer;
const music=Math.round(4*4*60/128*44100),total=music+88200;
const pcm=new Int16Array(total*2);let offset=0,peak=0,square=0;
const start=performance.now();
while(offset<total){
 if(offset===music)e.lxr_stop();
 const n=Math.min(128,(offset<music?music:total)-offset);
 e.lxr_process(n);assert.equal(e.memory.buffer,memory,'render grew WASM memory');
 const data=new Float32Array(memory,e.lxr_buffer(),n*2);
 for(let i=0;i<n*2;i++){const x=data[i];assert(Number.isFinite(x));peak=Math.max(peak,Math.abs(x));square+=x*x;pcm[offset*2+i]=Math.round(Math.max(-1,Math.min(1,x))*32767);}
 offset+=n;
}
const renderMs=performance.now()-start;
const native=await readFile(path.join(root,'artifacts/carbon-demo.wav'));
assert.equal(native.toString('ascii',0,4),'RIFF');assert.equal(native.toString('ascii',8,12),'WAVE');assert.equal(native.readUInt16LE(22),2);assert.equal(native.readUInt32LE(24),44100);assert.equal(native.readUInt32LE(40),pcm.length*2);
let maxDiff=0,diffSquare=0;
for(let i=0;i<pcm.length;i++){const d=pcm[i]-native.readInt16LE(44+i*2);maxDiff=Math.max(maxDiff,Math.abs(d));diffSquare+=d*d;}
const rmsDifference=Math.sqrt(diffSquare/pcm.length)/32768;
assert(maxDiff<=1,`native/WASM maximum error: ${maxDiff} PCM16 LSB`);
assert(rmsDifference<0.002,`native/WASM drift too large: ${rmsDifference}`);
const rom=await readFile(path.join(root,'src/data/lxr02-waves.bin'));
const provenance=JSON.parse(await readFile(path.join(root,'src/data/provenance.json'),'utf8'));
assert.equal(createHash('sha256').update(rom).digest('hex'),provenance.combined_sha256);
const report={wasmBytes:bytes.length,sampleRate:44100,voices:voiceEnergy.map(x=>Number(x.toFixed(3))),frames:total,peak,rms:Math.sqrt(square/(total*2)),renderMs,realTimeFactor:total/44100/(renderMs/1000),nativeWasmMaxDifferenceLSB:maxDiff,nativeWasmRmsDifference:rmsDifference,stableAudioMemory:true,allEffects:fxEnergy,oscillatorRomBytes:rom.length,oscillatorRomSha256:provenance.combined_sha256};
await mkdir(path.join(root,'artifacts'),{recursive:true});await writeFile(path.join(root,'artifacts/wasm-verification.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));
