import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {defs,EXTRA_DEFAULTS,bindDefinitions,hostParameterId,voiceParameterFromHost} from '../web/parameters.js';
import {migrateProject} from '../web/project-format.js';
import {featureCommands} from '../web/feature-state.js';

const module=await WebAssembly.compile(await readFile(new URL('../web/lxr02.wasm',import.meta.url)));
const core=new WebAssembly.Instance(module,{}).exports;
core.lxr_init(48000);bindDefinitions(core);
assert.equal(defs.length,56);
const ids=new Set();
for(let voice=0;voice<6;voice++)for(const d of defs){
 const id=hostParameterId(voice,d.id);ids.add(id);
 assert.deepEqual(voiceParameterFromHost(id),[voice,d.id]);
 if(d.id<39)assert.equal(id,voice*39+d.id);
 else assert.equal(core.lxr_get_param(voice,d.id),Math.fround(EXTRA_DEFAULTS[d.id-39]));
}
assert.equal(ids.size,336);assert.equal(Math.max(...ids),393);
assert.equal(core.lxr_param_max(14),7,'legacy filter automation must keep its range');
const legacy=JSON.parse(await readFile(new URL('../tests/fixtures/legacy-session.lxr02.json',import.meta.url)));
const state=migrateProject(legacy);
state.patterns[0].swings[3]=.2;
state.performance.routes[7]={source:12,sourceVoice:1,sourceParam:0,voice:3,param:47,amount:-.3,configured:true};
for(const [fn,args] of featureCommands(state,[0])){assert.equal(typeof core[fn],'function',fn);core[fn](...args);}
assert(Math.abs(core.lxr_track_get(0,3,4)-.2)<1e-6);
assert.equal(core.lxr_track_get(0,0,4),0);
console.log('PASS expanded WASM: 56 voice controls, stable legacy automation, cross-voice route commands and independent track swing.');
