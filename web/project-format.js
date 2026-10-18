import {upgradeFeatures} from './feature-state.js';
// Version 1 projects retain their notes, patterns and mix. Parameter units changed
// when the original DSP replaced the initial reconstruction.
export const FX_DEFAULTS=[0,0.22,0.25,0.35,0.55,0.25,0.8,0,1,0,0.25,0.5,0.75,4,1];
export function migrateProject(input){
 if(input?.format!=='lxr02-web'||input.version!==1)return upgradeFeatures(input);
 const s=structuredClone(input);
 const convert=(id,value,voice)=>id===8?(voice<3?value/16:Math.min(1,value)):id===25?value/2:id===34?Math.min(4,Math.max(0.25,value/3000)):value;
 if(Array.isArray(s.params))s.params=s.params.map((p,v)=>Array.isArray(p)&&p.length===36?[...p.map((x,id)=>convert(id,x,v)),0,0.22,4]:p);
 if(Array.isArray(s.patterns))for(const p of s.patterns)if(Array.isArray(p?.tracks))for(const [track,steps] of p.tracks.entries())if(Array.isArray(steps))for(const step of steps)if(Array.isArray(step?.locks))step.locks=step.locks.map(l=>Array.isArray(l)&&l.length===2?[l[0],convert(l[0],l[1],Math.min(track,5))]:l);
 if(Array.isArray(s.fx)&&s.fx.length===7){s.fx=[...s.fx,...FX_DEFAULTS.slice(7)];if(s.fx[0]===2)s.fx[2]=Math.min(0.7,(20+s.fx[2]*1800)/880*0.7);if(s.fx[0]===3)s.fx[12]=1+20*Math.log10(0.1+0.6*s.fx[4])/96;}
 s.version=2;return upgradeFeatures(s);
}
