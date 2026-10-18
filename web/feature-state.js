export const divisions=[['1/1',4],['1/2',2],['1/4',1],['1/8',.5],['1/16',.25],['1/32',.125],['1/64',.0625],['1/4 T',2/3],['1/8 T',1/3],['1/16 T',1/6],['1/32 T',1/12],['1/8 D',.75],['1/16 D',.375]];
export const conditionNames=['Always','First pass','After first pass','Fill','Not Fill','Previous hit','Previous missed','Every N passes'];
export const sourceNames=['Macro 1','Macro 2','Macro 3','Macro 4','Note velocity','Aftertouch','Mod wheel','LFO 1','LFO 2'];
export const grooveNames=['Straight','Swing','Broken','Push','Laid back'];
export function defaultPerformance(){return {macros:[0,0,0,0],macroNames:['Macro 1','Macro 2','Macro 3','Macro 4'],routes:Array.from({length:16},()=>({source:0,voice:0,param:12,amount:0})),velocityDepth:0,switchQuant:0,lfoPeriods:[4,1],mutation:{depth:.25,locked:[0,2,3,5,6,18,19,29]}};}
export function upgradeFeatures(s){
 if(s?.version!==2)return s;s=structuredClone(s);s.version=3;s.performance=defaultPerformance();s.scenes=Array(8).fill(null);s.samples=Array(7).fill(null);
 for(const p of s.patterns||[]){p.divisions=Array(7).fill(4);p.delays=Array(7).fill(0);p.grooves=Array(7).fill(0);p.grooveAmounts=Array(7).fill(0);for(const track of p.tracks||[])for(const step of track)Object.assign(step,{micro:0,condition:0,every:2,phase:0});}return s;
}
export function validateFeatures(raw,s,number,defs){
 const fail=()=>{throw new Error('Invalid performance data in project.');};
 const arr=(v,n,fn)=>{if(!Array.isArray(v)||v.length!==n)fail();return v.map(fn);};
 const bool=x=>{if(typeof x!=='boolean')fail();return x;};
 const params=p=>arr(p,6,a=>arr(a,39,(x,id)=>number(x,defs[id].min,defs[id].max)));
 const fxRanges=[[0,7],[0,1],[0,.743],[0,.94],[0,1],[0,1],[0,1.5],[0,1],[0,1],[0,5],[0,1],[0,1],[0,1],[1,8],[0,1]];
 s.performance={...defaultPerformance(),macros:arr(raw.performance?.macros,4,x=>number(x,0,1)),macroNames:arr(raw.performance?.macroNames,4,x=>{if(typeof x!=='string')fail();return x.slice(0,20);}),routes:arr(raw.performance?.routes,16,r=>({source:number(r.source,0,8)|0,voice:number(r.voice,0,5)|0,param:number(r.param,0,38)|0,amount:number(r.amount,-1,1)})),velocityDepth:number(raw.performance.velocityDepth,0,1),switchQuant:number(raw.performance.switchQuant,0,2)|0,lfoPeriods:arr(raw.performance.lfoPeriods,2,x=>number(x,.0625,32))};
 if(raw.performance.mutation){s.performance.mutation={depth:number(raw.performance.mutation.depth,0,1),locked:raw.performance.mutation.locked.filter(Number.isInteger).filter(id=>id>=0&&id<39)};}
 s.patterns.forEach((p,i)=>{const r=raw.patterns[i];for(const [field,min,max] of [['divisions',0,12],['delays',-.49,.49],['grooves',0,4],['grooveAmounts',0,1]])p[field]=arr(r[field],7,x=>number(x,min,max));p.tracks.forEach((track,t)=>track.forEach((step,k)=>{const x=r.tracks[t][k];Object.assign(step,{micro:number(x.micro,-.49,.49),condition:number(x.condition,0,7)|0,every:number(x.every,1,16)|0,phase:number(x.phase,0,15)|0});if(step.phase>=step.every)step.phase%=step.every;}));});
 s.scenes=arr(raw.scenes,8,x=>x===null?null:({enabled:bool(x.enabled),params:params(x.params),fx:arr(x.fx,15,(v,i)=>number(v,...fxRanges[i])),levels:arr(x.levels,7,v=>number(v,0,1.25)),muted:arr(x.muted,7,bool),solo:arr(x.solo,7,bool),macros:arr(x.macros,4,v=>number(v,0,1)),morph:number(x.morph,0,1),morphTarget:number(x.morphTarget,0,4)|0}));
 s.samples=arr(raw.samples,7,x=>{if(x===null)return null;if(typeof x.name!=='string'||typeof x.data!=='string'||x.data.length>2560000||x.data.length%4||!/^[A-Za-z0-9+/]*={0,2}$/.test(x.data))fail();const sample={name:x.name.slice(0,100),rate:number(x.rate,8000,192000),data:x.data,gain:number(x.gain,0,1.5),tune:number(x.tune,-36,36),start:number(x.start,0,.99),end:number(x.end,.001,1),decay:number(x.decay,.005,10),replace:bool(x.replace)};if(sample.end<=sample.start||atob(sample.data).length%2)fail();return sample;});
 return s;
}
export function featureCommands(s){
 const out=[],push=(fn,...args)=>out.push([fn,args]);const perf=s.performance;
 for(let p=0;p<8;p++)for(let t=0;t<7;t++){
  const x=s.patterns[p];push('lxr_track_settings',p,t,x.divisions[t],x.delays[t],x.grooves[t],x.grooveAmounts[t]);
  for(let k=0;k<64;k++){const a=x.tracks[t][k];push('lxr_step_extras',p,t,k,a.micro,a.condition,a.every,a.phase);}
 }
 for(let i=0;i<16;i++){const r=perf.routes[i];push('lxr_route',i,r.source,r.voice,r.param,r.amount);}
 perf.macros.forEach((v,i)=>push('lxr_macro',i,v));
 [[0,perf.switchQuant],[1,perf.velocityDepth],[5,perf.lfoPeriods[0]],[6,perf.lfoPeriods[1]]].forEach(a=>push('lxr_performance',...a));
 s.samples.forEach((a,t)=>{if(a)push('lxr_sample_settings',t,a.gain,a.tune,a.start,a.end,a.decay,+a.replace);});
 s.scenes.forEach((a,p)=>{
  if(a){a.params.forEach((row,v)=>row.forEach((x,id)=>push('lxr_scene_param',p,v,id,x)));a.fx.forEach((v,id)=>push('lxr_scene_fx',p,id,v));a.levels.forEach((v,t)=>{push('lxr_scene_mix',p,t,0,v);push('lxr_scene_mix',p,t,1,+a.muted[t]);push('lxr_scene_mix',p,t,2,+a.solo[t]);});a.macros.forEach((v,id)=>push('lxr_scene_misc',p,id,v));push('lxr_scene_misc',p,4,a.morph);push('lxr_scene_misc',p,5,a.morphTarget);}
  push('lxr_scene_enable',p,+!!a?.enabled);
 });
 return out;
}
export function pcmFromBase64(data){const bytes=Uint8Array.from(atob(data),x=>x.charCodeAt(0)),v=new DataView(bytes.buffer),pcm=new Float32Array(bytes.length/2);for(let i=0;i<pcm.length;i++)pcm[i]=Math.max(-1,v.getInt16(i*2,true)/32767);return pcm;}
export function base64FromBytes(bytes){let text='';for(let i=0;i<bytes.length;i+=8192)text+=String.fromCharCode(...bytes.subarray(i,i+8192));return btoa(text);}
export function pcmToBase64(pcm){const bytes=new Uint8Array(pcm.length*2),view=new DataView(bytes.buffer);pcm.forEach((x,i)=>view.setInt16(i*2,Math.round(Math.max(-1,Math.min(1,x))*32767),true));return base64FromBytes(bytes);}
export function uploadSamples(e,samples){samples.forEach((sample,t)=>{const pcm=sample?pcmFromBase64(sample.data):new Float32Array(0);const ptr=e.lxr_sample_alloc(pcm.length);if(pcm.length)new Float32Array(e.memory.buffer,ptr,pcm.length).set(pcm);e.lxr_sample_commit(t,sample?.rate??44100);});}
export function effective(s,key){const scene=s.scenes?.[s.pattern];return scene?.enabled?(key==='performance'?{...s.performance,macros:scene.macros}:scene[key]??s[key]):s[key];}
export function updateScene(s,key,value){const scene=s.scenes?.[s.pattern];if(scene?.enabled)scene[key]=structuredClone(value);}
export function captureScene(s){return {enabled:true,params:structuredClone(effective(s,'params')),fx:[...effective(s,'fx')],levels:[...effective(s,'levels')],muted:[...effective(s,'muted')],solo:[...effective(s,'solo')],macros:[...effective(s,'performance').macros],morph:effective(s,'morph'),morphTarget:effective(s,'morphTarget')};}
