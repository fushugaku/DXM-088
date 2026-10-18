import {divisions,conditionNames,sourceNames,grooveNames,effective,captureScene,pcmToBase64,pcmFromBase64,base64FromBytes} from './feature-state.js';
import {readWav,renderPattern,wavBytes,midiBytes,zipBytes} from './export-engine.js';
import {defs,tracks,normalize,denormalize,format} from './parameters.js';
export function createPerformanceUI(h){
 const {el,selectWidget,sliderField}=h,$=s=>document.querySelector(s);let tab='Timing',ready=null,busy=false,mutationBase=null,extraRoutes=new Set();
 const state=()=>h.state(),track=()=>h.track(),buttons=(labels,onClick)=>labels.map(([label,cls],i)=>{const b=el('button',cls||'outline-button',label);b.type='button';b.onclick=()=>onClick(i);return b;});
 const field=(label,node)=>{const r=el('div','feature-field');r.append(el('span','micro-label',label),node);return r;};
 function select(label,options,value,change){const root=el('div');selectWidget(root,options.map((x,i)=>typeof x==='string'?{label:x,value:i}:x),value,change,label);return field(label,root);}
 function edit(fn){h.remember();fn();h.save();}
 function syncTrack(){const s=state(),p=s.patterns[s.pattern],t=track();h.command('lxr_track_settings',s.pattern,t,p.divisions[t],p.delays[t],p.grooves[t],p.grooveAmounts[t]);h.save();}
 function timing(body){
  const s=state(),p=s.patterns[s.pattern],t=track(),grid=el('div','feature-grid timing-fields');
  grid.append(select('Groove',grooveNames,p.grooves[t],v=>edit(()=>{p.grooves[t]=v;syncTrack();render();})),sliderField('Groove amount',p.grooveAmounts[t]*100,0,100,1,v=>{p.grooveAmounts[t]=v/100;syncTrack();},v=>`${Math.round(v)}%`),sliderField('Track shift',p.delays[t]*100,-49,49,1,v=>{p.delays[t]=v/100;syncTrack();},v=>`${v>0?'+':''}${Math.abs(v)>0&&Math.abs(v)<10?v.toFixed(1):Math.round(v)}%`));
  grid.querySelector('[aria-label="Groove amount"]').disabled=p.grooves[t]===0;
  const summary=el('div','timing-summary'),beats=p.lengths[t]*divisions[p.divisions[t]][1],hits=p.tracks[t].slice(0,p.lengths[t]).filter(a=>a.v>0).length;
  summary.append(el('span','',tracks[t]),el('span','micro-label',`${p.lengths[t]} × ${divisions[p.divisions[t]][0]} · ${hits} hits · ${Number((beats/4).toFixed(3))} ${beats===4?'bar':'bars'}`));body.append(summary,grid);
 }
 function perform(body){
  const s=state(),perf=s.performance,macros=effective(s,'performance').macros,grid=el('div','macro-grid');
  for(let i=0;i<4;i++){
   const field=sliderField(perf.macroNames[i],macros[i]*100,0,100,.1,v=>{perf.macros[i]=v/100;macros[i]=v/100;h.command('lxr_macro',i,v/100);h.save();},v=>`${Math.round(v)}%`);field.dataset.macro=i;
   field.querySelector('.field-label>span').ondblclick=()=>{const input=el('input');input.type='text';input.maxLength=20;input.value=perf.macroNames[i];field.prepend(input);input.focus();input.select();input.onkeydown=e=>{if(e.key==='Enter')input.blur();if(e.key==='Escape'){input.value=perf.macroNames[i];input.blur();}};input.onblur=()=>{perf.macroNames[i]=input.value.trim()||`Macro ${i+1}`;h.publish();render();};};grid.append(field);
  }
  const controls=el('div','feature-grid perform-options');
  const fill=el('button','outline-button fill-button','FILL');fill.setAttribute('aria-label','Hold Fill');fill.setAttribute('aria-pressed','false');const setFill=v=>{h.command('lxr_performance',2,+v);fill.setAttribute('aria-pressed',v);};fill.onpointerdown=e=>{e.preventDefault();fill.setPointerCapture(e.pointerId);setFill(true);};fill.onpointerup=fill.onpointercancel=fill.onlostpointercapture=()=>setFill(false);fill.onkeydown=e=>{if([' ','Enter'].includes(e.key)){e.preventDefault();e.stopPropagation();setFill(true);}};fill.onkeyup=e=>{if([' ','Enter'].includes(e.key)){e.preventDefault();e.stopPropagation();setFill(false);}};fill.onblur=()=>setFill(false);
  controls.append(select('Pattern change',['Immediate','Next 1/16','Next bar'],perf.switchQuant,v=>edit(()=>{perf.switchQuant=v;h.command('lxr_performance',0,v);h.save();})),sliderField('Velocity influence',perf.velocityDepth*100,0,100,1,v=>{perf.velocityDepth=v/100;h.command('lxr_performance',1,v/100);h.save();},v=>`${Math.round(v)}%`),field('Momentary · CC64',fill));
  body.append(grid,controls);
 }
 function scenes(body){
  const s=state(),list=el('div','scene-slots');
  for(let p=0;p<8;p++){const scene=s.scenes[p],b=el('button',`scene-slot${p===s.pattern?' selected':''}`);b.append(el('b','',`A${String(p+1).padStart(2,'0')}`),el('span','micro-label',scene?.enabled?'SCENE':'SHARED'));b.setAttribute('aria-label',`Scene A${String(p+1).padStart(2,'0')}`);b.onclick=()=>h.selectPattern(p);list.append(b);}
  const actions=el('div','feature-actions');actions.append(...buttons([['Capture sound + mix','primary-button'],['Use shared sound']],i=>edit(()=>{s.scenes[s.pattern]=i===0?captureScene(s):null;h.publish();h.renderAll();h.toast(i===0?'Scene captured':'Shared sound selected');})));
  body.append(list,actions);
 }
 function matrix(body){
  const s=state(),list=el('div','mod-list'),active=s.performance.routes.map((r,i)=>[r,i]).filter(([r,i])=>r.amount!==0||extraRoutes.has(i));
  for(const [r,i] of active){
   const row=el('div','mod-row');const update=()=>{h.command('lxr_route',i,r.source,r.voice,r.param,r.amount);h.save();};
   row.append(select(`Source ${i+1}`,sourceNames.map((x,k)=>({value:k,label:k<4?s.performance.macroNames[k]:x})),r.source,v=>edit(()=>{r.source=v;update();})),select(`Voice ${i+1}`,tracks.slice(0,6).map((label,value)=>({label,value})),r.voice,v=>edit(()=>{r.voice=v;update();})),select(`Destination ${i+1}`,defs.map(d=>({label:d.name,value:d.id})),r.param,v=>edit(()=>{r.param=v;update();})),sliderField('Depth',r.amount*100,-100,100,.1,v=>{r.amount=v/100;update();},v=>`${v>0?'+':''}${Math.abs(v)>0&&Math.abs(v)<10?v.toFixed(1):Math.round(v)}%`));
   const remove=el('button','icon-button','×');remove.setAttribute('aria-label',`Remove route ${i+1}`);remove.onclick=()=>edit(()=>{r.amount=0;extraRoutes.delete(i);update();render();});row.append(remove);list.append(row);
  }
  const add=el('button','outline-button','+ Modulation route');add.disabled=active.length===16;add.onclick=()=>{const i=s.performance.routes.findIndex((r,i)=>r.amount===0&&!extraRoutes.has(i));if(i>=0){extraRoutes.add(i);render();}};
  const lfos=el('div','lfo-controls');for(let i=0;i<2;i++)lfos.append(select(`LFO ${i+1} cycle`,[.125,.25,.5,1,2,4,8,16,32].map(v=>({value:v,label:`${v} beats`})),s.performance.lfoPeriods[i],v=>edit(()=>{s.performance.lfoPeriods[i]=v;h.command('lxr_performance',5+i,v);h.save();})));
  if(!active.length)list.append(el('p','secondary','Route macros, MIDI expression or tempo LFOs to voice parameters.'));
  body.append(list,el('div','feature-actions'));body.lastElementChild.append(add,lfos);
 }
 function sample(body){
  const s=state(),t=track(),a=s.samples[t],header=el('div','sample-heading'),name=el('span','sample-name',a?.name||`${tracks[t]} · No sample`),file=el('input');file.type='file';file.accept='.wav,audio/wav,audio/x-wav';file.hidden=true;
  const load=el('button','outline-button','Load WAV');load.onclick=()=>file.click();file.onchange=async()=>{const f=file.files[0];if(!f)return;try{if(f.size>16_000_000)throw new Error('WAV file is too large.');const decoded=readWav(await f.arrayBuffer());edit(()=>{s.samples[t]={name:f.name,rate:decoded.rate,data:pcmToBase64(decoded.pcm),gain:.7,tune:0,start:0,end:1,decay:4,replace:false};h.syncSamples();h.publish();render();});}catch(e){h.report(e);}};
  const clear=el('button','text-button','Remove');clear.disabled=!a;clear.onclick=()=>edit(()=>{s.samples[t]=null;h.syncSamples();h.publish();render();});header.append(name,load,clear,file);body.append(header);
  if(!a)return;
  const canvas=el('canvas','sample-wave');canvas.width=1000;canvas.height=80;canvas.setAttribute('aria-label','Sample waveform');const pcm=pcmFromBase64(a.data),ctx=canvas.getContext('2d'),peaks=new Float32Array(1000);
  for(let x=0;x<1000;x++){const start=Math.floor(x*pcm.length/1000),end=Math.max(start+1,Math.floor((x+1)*pcm.length/1000));for(let i=start;i<Math.min(pcm.length,end);i++)peaks[x]=Math.max(peaks[x],Math.abs(pcm[i]));}
  const drawWave=()=>{ctx.clearRect(0,0,1000,80);ctx.strokeStyle='#aaa99f';ctx.fillStyle='#eab46022';ctx.fillRect(a.start*1000,0,(a.end-a.start)*1000,80);ctx.beginPath();for(let x=0;x<1000;x++){ctx.moveTo(x,40-peaks[x]*36);ctx.lineTo(x,40+peaks[x]*36);}ctx.stroke();};drawWave();body.append(canvas);
  const controls=el('div','feature-grid sample-controls'),update=()=>{drawWave();const start=controls.querySelector('[aria-label=Start]'),end=controls.querySelector('[aria-label=End]');if(start&&end){start.value=a.start*100;start.max=a.end*100-.1;end.value=a.end*100;end.min=a.start*100+.1;start.closest('.field').querySelector('output').textContent=`${(a.start*100).toFixed(1)}%`;end.closest('.field').querySelector('output').textContent=`${(a.end*100).toFixed(1)}%`;}h.command('lxr_sample_settings',t,a.gain,a.tune,a.start,a.end,a.decay,+a.replace);h.save();};
  controls.append(select('Layer mode',['Synth + sample','Sample only'],+a.replace,v=>edit(()=>{a.replace=!!v;update();})),sliderField('Sample level',a.gain*100,0,150,1,v=>{a.gain=v/100;update();},v=>`${v}%`),sliderField('Tune',a.tune,-36,36,.1,v=>{a.tune=v;update();},v=>`${v>0?'+':''}${v.toFixed(1)} st`),sliderField('Start',a.start*100,0,99,.1,v=>{a.start=Math.min(v/100,a.end-.001);update();},v=>`${v.toFixed(1)}%`),sliderField('End',a.end*100,.1,100,.1,v=>{a.end=Math.max(v/100,a.start+.001);update();},v=>`${v.toFixed(1)}%`),sliderField('Sample decay',a.decay,.005,10,.005,v=>{a.decay=v;update();},v=>`${v.toFixed(2)} s`));body.append(controls);
 }
 function mutate(body){
  const s=state(),m=s.performance.mutation,v=Math.min(track(),5),controls=el('div','mutate-controls');controls.append(sliderField('Mutation depth',m.depth*100,0,100,1,x=>{m.depth=x/100;h.save();},x=>`${x}%`));
  const actions=el('div','feature-actions'),mutate=el('button','primary-button',`Mutate ${tracks[track()]}`),revert=el('button','outline-button','Revert');revert.disabled=!mutationBase||mutationBase.voice!==v;
  mutate.onclick=()=>{h.remember();const row=effective(s,'params')[v];if(!mutationBase||mutationBase.voice!==v)mutationBase={voice:v,params:[...row]};for(const d of defs){if(m.locked.includes(d.id))continue;const delta=(Math.random()*2-1)*m.depth*.4;if(d.options&&Math.random()>m.depth)continue;const value=denormalize(d,Math.max(0,Math.min(1,normalize(d,row[d.id])+delta)));row[d.id]=value;h.command('lxr_param',v,d.id,value);}h.renderKnobs();h.save();render();};
  revert.onclick=()=>{if(mutationBase?.voice!==v)return;h.remember();mutationBase.params.forEach((value,id)=>{effective(s,'params')[v][id]=value;h.command('lxr_param',v,id,value);});mutationBase=null;h.renderKnobs();h.save();render();};actions.append(mutate,revert);controls.append(actions);body.append(controls,el('span','micro-label','Protected parameters'));
  const protectedParams=el('div','protected-params');for(const d of defs){const b=el('button',m.locked.includes(d.id)?'protected':'',d.name);b.setAttribute('aria-pressed',m.locked.includes(d.id));b.onclick=()=>{m.locked=m.locked.includes(d.id)?m.locked.filter(x=>x!==d.id):[...m.locked,d.id];b.classList.toggle('protected',m.locked.includes(d.id));b.setAttribute('aria-pressed',m.locked.includes(d.id));h.save();};protectedParams.append(b);}body.append(protectedParams);
 }
 let exportBars=4,exportMode=0;
 async function prepareExport(kind){
  if(busy)return;busy=true;ready=null;render();const s=structuredClone(state()),beats=exportBars*4,base=`${h.filename()}-a${String(s.pattern+1).padStart(2,'0')}`,renderedBars=exportBars,progress=$('#export-progress');
  try{
   if(!(kind==='midi'&&exportMode===1)&&beats*60/s.bpm>120)throw new Error('Choose fewer bars: maximum render length is 120 seconds.');
   const commands=h.snapshot(s),progressText=x=>{if(progress)progress.textContent=`Rendering ${Math.round(x*100)}%`;};let files;
   if(kind==='stems'){
    files=[];for(let t=0;t<7;t++){const result=await renderPattern(h.module(),commands,s,{beats,track:t,onProgress:x=>progressText((t+x)/7)});files.push({name:`${base}-${t+1}-${tracks[t].toLowerCase().replace(/[^a-z0-9]+/g,'-')}.wav`,mime:'audio/wav',bytes:wavBytes(result.pcm,result.sr)});}
   }else if(kind==='midi'&&exportMode===1){files=[{name:`${base}-gate.mid`,mime:'audio/midi',bytes:midiBytes([],s.bpm,beats,{gateNote:(s.plugin?.baseNote??24)+s.pattern})}];}
   else {const result=await renderPattern(h.module(),commands,s,{beats,audio:kind==='wav',onProgress:progressText});files=[{name:`${base}.${kind==='wav'?'wav':'mid'}`,mime:kind==='wav'?'audio/wav':'audio/midi',bytes:kind==='wav'?wavBytes(result.pcm,result.sr):midiBytes(result.hits,s.bpm,beats,{names:tracks})}];}
   ready={files,kind,label:`A${String(s.pattern+1).padStart(2,'0')} · ${renderedBars} ${renderedBars===1?'bar':'bars'}`,token:null};
   if(h.native&&!h.native.preview&&h.native.supportsFileDrag)h.native.prepareFiles(files.map(f=>({name:f.name,data:base64FromBytes(f.bytes)})));
  }catch(e){h.report(e);}finally{busy=false;render();}
 }
 function exports(body){
  const controls=el('div','export-controls'),bars=el('input');bars.type='number';bars.min=1;bars.max=64;bars.value=exportBars;bars.setAttribute('aria-label','Export bars');bars.oninput=()=>{if(!bars.value)return;exportBars=Math.max(1,Math.min(64,Math.round(+bars.value||4)));bars.value=exportBars;};
  controls.append(field('Bars · 4/4',bars),select('MIDI content',['Drum notes','Pattern gate'],exportMode,v=>exportMode=v));
  const actions=el('div','feature-actions');for(const [kind,label] of [['wav','Render WAV'],['midi','Export MIDI'],['stems','Render 7 stems']]){const b=el('button','outline-button',label);b.disabled=busy;b.onclick=()=>prepareExport(kind);actions.append(b);}controls.append(actions);body.append(controls);
  const status=el('p','secondary',busy?'Rendering…':ready?ready.label:'');status.id='export-progress';body.append(status);
  if(ready){
   const result=el('div','export-result'),drag=el('button','primary-button',`↗ Drag ${ready.kind==='stems'?'7 WAV files':ready.kind.toUpperCase()}`);drag.draggable=true;drag.hidden=!!h.native&&!h.native.preview&&!h.native.supportsFileDrag;drag.disabled=!!h.native&&!h.native.preview&&h.native.supportsFileDrag&&!ready.token;
   drag.onpointerdown=e=>{if(h.native&&!h.native.preview&&ready.token){e.preventDefault();h.native.dragFiles(ready.token);}};
   drag.ondragstart=e=>{if(h.native&&!h.native.preview){e.preventDefault();return;}ready.files.forEach(f=>e.dataTransfer.items.add(new File([f.bytes],f.name,{type:f.mime})));const f=ready.files[0];e.dataTransfer.setData('DownloadURL',`${f.mime}:${f.name}:${URL.createObjectURL(new Blob([f.bytes],{type:f.mime}))}`);e.dataTransfer.effectAllowed='copy';};
   const save=el('button','outline-button',ready.kind==='stems'?'Download ZIP':'Save file');save.onclick=()=>{const f=ready.files[0];h.download(new Blob([ready.kind==='stems'?zipBytes(ready.files):f.bytes],{type:ready.kind==='stems'?'application/zip':f.mime}),ready.kind==='stems'?`${h.filename()}-stems.zip`:f.name);};result.append(drag,save);body.append(result);
  }
 }
 const renderers={Timing:timing,Perform:perform,Scenes:scenes,Matrix:matrix,Sample:sample,Mutate:mutate,Export:exports};
 function render(){
  const tabs=$('#feature-tabs');tabs.replaceChildren(...Object.keys(renderers).map(name=>{const b=el('button',name===tab?'active':'',name.toUpperCase());b.setAttribute('aria-selected',name===tab);b.setAttribute('role','tab');b.onclick=()=>{if(tab==='Perform')h.command('lxr_performance',2,0);tab=name;render();};return b;}));
  const body=$('#feature-body');body.replaceChildren();body.setAttribute('aria-label',tab);renderers[tab](body);
 }
 addEventListener('lxr-native-export-error',({detail})=>h.toast(detail.message));
 addEventListener('lxr-native-files',({detail})=>{if(ready){ready.token=detail.token;render();}});
 function refresh(){if(tab==='Perform'){const s=state(),macros=effective(s,'performance').macros;for(let i=0;i<4;i++){const r=$(`[data-macro="${i}"]`);if(r){r.querySelector('input[type=range]').value=macros[i]*100;r.querySelector('output').textContent=`${Math.round(macros[i]*100)}%`;}}}}
 return {render,refresh,showExport(){tab='Export';render();$('#performance-panel').scrollIntoView({behavior:'smooth',block:'nearest'});}};
}
