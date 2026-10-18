import {EXTRA_DEFAULTS} from './parameters.js';
import {upgradeFeatures} from './feature-state.js';
import {upgradePatternBank} from './pattern-bank.js';
// Version 1 projects retain their notes, patterns and mix. Parameter units changed
// when the original DSP replaced the initial reconstruction.
export const FX_DEFAULTS=[0,0.22,0.25,0.35,0.55,0.25,0.8,0,1,0,0.25,0.5,0.75,4,1];
export const DEFAULT_DRUM_NOTES=[36,45,47,38,49,42,46];
export function defaultPlugin(){return {version:3,baseNote:0,midiMode:0,outputMode:0,drumNotes:[...DEFAULT_DRUM_NOTES]};}
export function migrateProject(input){
 if(input?.format!=='lxr02-web'||input.version!==1)return upgradeSynthesis(upgradePatternBank(upgradeFeatures(input)));
 const s=structuredClone(input);
 const convert=(id,value,voice)=>id===8?(voice<3?value/16:Math.min(1,value)):id===25?value/2:id===34?Math.min(4,Math.max(0.25,value/3000)):value;
 if(Array.isArray(s.params))s.params=s.params.map((p,v)=>Array.isArray(p)&&p.length===36?[...p.map((x,id)=>convert(id,x,v)),0,0.22,4]:p);
 if(Array.isArray(s.patterns))for(const p of s.patterns)if(Array.isArray(p?.tracks))for(const [track,steps] of p.tracks.entries())if(Array.isArray(steps))for(const step of steps)if(Array.isArray(step?.locks))step.locks=step.locks.map(l=>Array.isArray(l)&&l.length===2?[l[0],convert(l[0],l[1],Math.min(track,5))]:l);
 if(Array.isArray(s.fx)&&s.fx.length===7){s.fx=[...s.fx,...FX_DEFAULTS.slice(7)];if(s.fx[0]===2)s.fx[2]=Math.min(0.7,(20+s.fx[2]*1800)/880*0.7);if(s.fx[0]===3)s.fx[12]=1+20*Math.log10(0.1+0.6*s.fx[4])/96;}
 s.version=2;return upgradeSynthesis(upgradePatternBank(upgradeFeatures(s)));
}

// Extend old sound arrays and add neutral per-track swing without changing inputs.
export function upgradeSynthesis(input){
 if(input?.version!==4||!Array.isArray(input.params)||!Array.isArray(input.patterns))return input;
 const oldRows=input.params.some(row=>row?.length===39)||input.scenes?.some(scene=>scene?.params?.some(row=>row?.length===39));
 const oldTracks=input.patterns.some(p=>!p.swings);
 const oldRoutes=input.performance?.routes?.some(r=>r.sourceVoice===undefined);
 if(!oldRows&&!oldTracks&&!oldRoutes)return input;
 const s=structuredClone(input),extend=row=>row?.length===39?[...row,...EXTRA_DEFAULTS]:row;
 s.params=s.params.map(extend);s.scenes=s.scenes?.map(scene=>scene?{...scene,params:scene.params.map(extend)}:null);
 for(const p of s.patterns)if(!p.swings)p.swings=Array(7).fill(0);
 if(s.performance?.routes)s.performance.routes=s.performance.routes.map(r=>({sourceVoice:r.voice,sourceParam:12,configured:r.amount!==0,...r}));
 return s;
}
