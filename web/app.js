import {readStoredProject,storeProject} from './project-storage.js';
import {upgradeFeatures,validateFeatures,featureCommands,uploadSamples,effective,updateScene,divisions,conditionNames,defaultPerformance} from './feature-state.js';
import {createPerformanceUI} from './performance-ui.js';
import {migrateProject,FX_DEFAULTS} from './project-format.js';
import {kits,tracks,shortTracks,keys,defs,pages,clamp,bindDefinitions,normalize,denormalize,constrain,format} from './parameters.js';
import {KnobDrag,wheelPixels,ParameterEdits} from './knob-input.js';

const $=s=>document.querySelector(s),$$=s=>[...document.querySelectorAll(s)];
const el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
const storageKey='lxr02-project-v1';
const native=globalThis.lxrNative;let receivingNative=false,nativeBooted=false;
let module,core,state,factoryKits,audio=null,audioPending=null,playing=false,selectedTrack=0,page=0,bar=0,positions=Array(7).fill(-1),audioPattern=0,editedStep=null,dirtyTimer,toastTimer;
let past=[],knobUpdates=[],tapTimes=[],suppressStepClick=false;
let kitSelect,fxSelect,features,divisionSelect;
const waveform=new Float32Array(1024);
const localEdits=new ParameterEdits();
function markLocalCommand(fn,args){
 let id,value;
 if(fn==='lxr_param'){id=args[0]*39+args[1];value=args[2];}
 else if(fn==='lxr_fx'){id=234+args[0];value=args[1];}
 else if(fn==='lxr_level'){id=249+args[0];value=args[1];}
 else if(fn==='lxr_mute'||fn==='lxr_solo'){id=(fn==='lxr_mute'?256:263)+args[0];value=args[1];}
 else if(fn==='lxr_tempo'){id=270;value=args[1];}
 else if(fn==='lxr_macro'){id=274+args[0];value=args[1];}
 else if(fn==='lxr_performance'&&[0,1,2,5,6].includes(args[0])){id=({0:280,1:278,2:279,5:281,6:282})[args[0]];value=args[1];}
 else if(fn==='lxr_morph'){localEdits.write(272,args[1],performance.now());id=271;value=args[0];}
 if(id!==undefined)localEdits.write(id,value,performance.now());
}

function toast(message){$('#toast').textContent=message;$('#toast').classList.add('visible');clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('#toast').classList.remove('visible'),3200);}
function report(error){console.error(error);toast(error instanceof Error?error.message:String(error));}
function saveSoon(){if(native){native.meta(state.name,state.kit,{macroNames:state.performance.macroNames,mutation:state.performance.mutation});return;}clearTimeout(dirtyTimer);dirtyTimer=setTimeout(()=>{storeProject(state).catch(()=>{$('#engine-info').textContent='Browser storage unavailable · download your project';});},250);}
function remember(){past.push({json:JSON.stringify({...state,samples:state.samples.map(a=>a?{...a,data:undefined}:null)}),samples:state.samples.map(a=>a?.data??null)});if(past.length>32)past.shift();$('#undo').disabled=false;}
function command(fn,...args){
 if(fn==='lxr_param')state.params[args[0]][args[1]]=args[2];
 else if(fn==='lxr_fx')state.fx[args[0]]=args[1];
 else if(fn==='lxr_level')state.levels[args[0]]=args[1];
 else if(fn==='lxr_mute')state.muted[args[0]]=!!args[1];
 else if(fn==='lxr_solo')state.solo[args[0]]=!!args[1];
 core[fn](...args);if(audio)audio.node.port.postMessage({command:[fn,args]});if(native&&!receivingNative){markLocalCommand(fn,args);native.commands([[fn,args]]);}}
function batch(commands){for(const [fn,args] of commands)core[fn](...args);if(audio)audio.node.port.postMessage({batch:commands});if(native&&!receivingNative){if(commands.length>64)native.project({...state,plugin:{version:1,baseNote:Number($('#pattern-note-base')?.value??24)}});else native.commands(commands);}}

function readParams(){return Array.from({length:6},(_,v)=>Array.from({length:defs.length},(_,p)=>core.lxr_get_param(v,p)));}
function readPattern(p){return {lengths:Array.from({length:7},(_,t)=>core.lxr_get_length(p,t)),divisions:Array.from({length:7},(_,t)=>core.lxr_track_get(p,t,0)),delays:Array.from({length:7},(_,t)=>core.lxr_track_get(p,t,1)),grooves:Array.from({length:7},(_,t)=>core.lxr_track_get(p,t,2)),grooveAmounts:Array.from({length:7},(_,t)=>core.lxr_track_get(p,t,3)),tracks:Array.from({length:7},(_,t)=>Array.from({length:64},(_,k)=>({v:core.lxr_get_step(p,t,k,0),prob:core.lxr_get_step(p,t,k,1),note:core.lxr_get_step(p,t,k,2),ratchet:core.lxr_get_step(p,t,k,3),micro:core.lxr_get_step(p,t,k,8),condition:core.lxr_get_step(p,t,k,9),every:core.lxr_get_step(p,t,k,10),phase:core.lxr_get_step(p,t,k,11),locks:[[core.lxr_get_step(p,t,k,4),core.lxr_get_step(p,t,k,5)],[core.lxr_get_step(p,t,k,6),core.lxr_get_step(p,t,k,7)]]})))};}
function freshState(){return {format:'lxr02-web',version:3,name:'Session 01',kit:0,bpm:128,swing:0,params:readParams(),patterns:Array.from({length:8},(_,p)=>readPattern(p)),pattern:0,levels:Array(7).fill(0.85),muted:Array(7).fill(false),solo:Array(7).fill(false),fx:[...FX_DEFAULTS],morph:0,morphTarget:1,performance:defaultPerformance(),scenes:Array(8).fill(null),samples:Array(7).fill(null)};}
function syncSamples(){uploadSamples(core,state.samples);if(audio)audio.node.port.postMessage({samples:state.samples});}
function publishProject(){const wasPlaying=playing;batch(snapshotCommands(state));syncSamples();if(wasPlaying&&!native)command('lxr_play');saveSoon();}
function selectPattern(p){state.pattern=p;command('lxr_pattern',p);$('#pattern-label').textContent=`A${String(p+1).padStart(2,'0')}`;renderAll();saveSoon();}
function snapshotCommands(s){
 const list=[['lxr_panic',[]],['lxr_tempo',[s.bpm,s.swing]],['lxr_pattern',[s.pattern]]];
 for(let v=0;v<6;v++)for(let p=0;p<defs.length;p++)list.push(['lxr_param',[v,p,s.params[v][p]]]);
 for(let p=0;p<8;p++)for(let t=0;t<7;t++){
  list.push(['lxr_length',[p,t,s.patterns[p].lengths[t]]]);
  for(let i=0;i<64;i++){
   const a=s.patterns[p].tracks[t][i];list.push(['lxr_set_step',[p,t,i,a.v,a.prob,a.note,a.ratchet]]);
   for(let slot=0;slot<2;slot++)list.push(['lxr_set_lock',[p,t,i,slot,...a.locks[slot]]]);
  }
 }
 for(let t=0;t<7;t++){list.push(['lxr_level',[t,s.levels[t]]],['lxr_mute',[t,+s.muted[t]]],['lxr_solo',[t,+s.solo[t]]]);}
 for(let p=0;p<s.fx.length;p++)list.push(['lxr_fx',[p,s.fx[p]]]);
 list.push(['lxr_morph',[s.morph,s.morphTarget]],...featureCommands(s));return list;
}
function validateProject(input){
 const raw=migrateProject(input);
 const fail=()=>{throw new Error('This file is not a valid LXR / 02 project.');};
 const number=(v,min,max)=>{if(!Number.isFinite(v)||v<min-0.0001||v>max+0.0001)fail();return clamp(v,min,max);};
 if(raw?.format!=='lxr02-web'||raw.version!==3)fail();
 const s={format:'lxr02-web',version:3,name:typeof raw.name==='string'?raw.name.slice(0,60):'Session 01',kit:number(raw.kit,0,4)|0,bpm:number(raw.bpm,30,300),swing:number(raw.swing,0,0.5),pattern:number(raw.pattern,0,7)|0,morph:number(raw.morph,0,1),morphTarget:number(raw.morphTarget,0,4)|0};
 if(!Array.isArray(raw.params)||raw.params.length!==6||!Array.isArray(raw.patterns)||raw.patterns.length!==8)fail();
 s.params=raw.params.map(a=>{if(!Array.isArray(a)||a.length!==defs.length)fail();return a.map((x,i)=>constrain(defs[i],number(x,defs[i].min,defs[i].max)));});
 s.patterns=raw.patterns.map(p=>{
  if(!Array.isArray(p.lengths)||p.lengths.length!==7||!Array.isArray(p.tracks)||p.tracks.length!==7)fail();
  return {lengths:p.lengths.map(n=>number(n,1,64)|0),tracks:p.tracks.map(a=>{
   if(!Array.isArray(a)||a.length!==64)fail();
   return a.map(x=>{
    if(!Array.isArray(x.locks)||x.locks.length!==2)fail();
    return {v:number(x.v,0,1),prob:number(x.prob,0,1),note:number(x.note,-36,36)|0,ratchet:number(x.ratchet,1,4)|0,locks:x.locks.map(l=>{if(!Array.isArray(l)||l.length!==2)fail();const id=l[0];if(id===255)return [255,0];if(!Number.isInteger(id)||id<0||id>=defs.length)fail();return [id,number(l[1],defs[id].min,defs[id].max)];})};
   });
  })};
 });
 for(const field of ['levels','muted','solo']){if(!Array.isArray(raw[field])||raw[field].length!==7)fail();s[field]=raw[field].map(x=>field==='levels'?number(x,0,1.25):Boolean(x));}
 if(!Array.isArray(raw.fx)||raw.fx.length!==15)fail();
 const ranges=[[0,7],[0,1],[0,0.743],[0,0.94],[0,1],[0,1],[0,1.5],[0,1],[0,1],[0,5],[0,1],[0,1],[0,1],[1,8],[0,1]];
 s.fx=raw.fx.map((x,i)=>number(x,...ranges[i]));if(raw.plugin)s.plugin={version:1,baseNote:number(raw.plugin.baseNote,0,120)|0};return validateFeatures(raw,s,number,defs);
}

let activeDropdown=null,dropdownSerial=0;
function selectWidget(container,options,value,onChange,label){
 const root=el('div','dropdown');const trigger=el('button','select-trigger');trigger.type='button';trigger.setAttribute('aria-label',label);trigger.setAttribute('aria-haspopup','listbox');trigger.setAttribute('aria-expanded','false');
 const current=el('span'),arrow=el('span','chevron','⌄');trigger.append(current,arrow);
 const menu=el('div','dropdown-menu');menu.id=`dropdown-${++dropdownSerial}`;menu.hidden=true;menu.setAttribute('role','listbox');menu.setAttribute('aria-label',label);trigger.setAttribute('aria-controls',menu.id);
 let selected=value;
 const searchable=options.length>16;let search=null;
 const optionNodes=()=>[...menu.querySelectorAll('[role="option"]')].filter(b=>!b.hidden);
 function set(v){selected=v;current.textContent=options.find(o=>o.value===v)?.label??'—';for(const b of menu.querySelectorAll('[role=option]'))b.setAttribute('aria-selected',String(+b.dataset.value===v));}
 function close(){menu.hidden=true;trigger.setAttribute('aria-expanded','false');root.append(menu);if(activeDropdown===control)activeDropdown=null;removeEventListener('resize',place);visualViewport?.removeEventListener('resize',place);document.removeEventListener('scroll',onScroll,true);root.closest('dialog')?.removeEventListener('close',close);}
 function focusOption(option){if(!option)return;option.focus({preventScroll:true});const top=option.offsetTop-(search?search.offsetHeight+4:0),bottom=option.offsetTop+option.offsetHeight;if(top<menu.scrollTop)menu.scrollTop=top;else if(bottom>menu.scrollTop+menu.clientHeight)menu.scrollTop=bottom-menu.clientHeight;}
 function place(){
  if(!trigger.isConnected){close();return;}
  const rect=trigger.getBoundingClientRect(),view=visualViewport,pad=8,gap=4;
  const left=view?.offsetLeft??0,top=view?.offsetTop??0,width=view?.width??innerWidth,height=view?.height??innerHeight;
  const below=top+height-pad-rect.bottom-gap,above=rect.top-top-pad-gap;
  menu.style.width=`${Math.min(Math.max(rect.width,208),width-pad*2)}px`;
  menu.style.left=`${clamp(rect.left,left+pad,left+width-pad-parseFloat(menu.style.width))}px`;
  const desired=Math.min(menu.scrollHeight+2,280),up=below<desired&&above>below;
  menu.style.maxHeight=`${Math.max(0,Math.min(desired,up?above:below))}px`;
  menu.style.top=`${up?rect.top-gap-menu.getBoundingClientRect().height:rect.bottom+gap}px`;
 }
 function onScroll(event){if(!menu.contains(event.target))place();}
 function open(){
  activeDropdown?.close();(root.closest('dialog[open]')||document.body).append(menu);menu.hidden=false;trigger.setAttribute('aria-expanded','true');activeDropdown=control;place();
  if(search){search.value='';filterOptions();search.focus({preventScroll:true});}else focusOption(menu.querySelector('[aria-selected="true"]')||optionNodes()[0]);
  addEventListener('resize',place);visualViewport?.addEventListener('resize',place);document.addEventListener('scroll',onScroll,true);root.closest('dialog')?.addEventListener('close',close,{once:true});
 }
 const group=id=>[0,1,10,26,32,35].includes(id)?'Oscillator':[2,3,4,5,6,28,29,37].includes(id)?'Envelope':[7,8,9,24,25,38].includes(id)?'FM':[12,13,14,15,16,17,36].includes(id)?'Filter & drive':[11,27,34].includes(id)?'Transient':[18,19].includes(id)?'Mixer':'Modulation';
 const grouped=searchable?options.map(o=>({...o,group:o.value===255?'':group(o.value)})).sort((a,b)=>{const order=['','Oscillator','Envelope','FM','Filter & drive','Transient','Modulation','Mixer'];return order.indexOf(a.group)-order.indexOf(b.group);}):options;
 if(searchable){search=el('input','dropdown-search');search.type='text';search.placeholder='Search parameter';search.setAttribute('aria-label',`Search ${label}`);search.autocomplete='off';menu.append(search);search.oninput=()=>{filterOptions();place();};}
 let lastGroup=null;
 for(const option of grouped){
  if(option.group&&option.group!==lastGroup){const heading=el('div','dropdown-group',option.group);heading.dataset.group=option.group;menu.append(heading);lastGroup=option.group;}
  const button=el('button','',option.label);button.type='button';button.tabIndex=-1;button.dataset.value=option.value;button.dataset.group=option.group||'';button.setAttribute('role','option');button.onclick=()=>{set(option.value);close();trigger.focus({preventScroll:true});onChange(option.value);};menu.append(button);
 }
 function filterOptions(){const query=search?.value.trim().toLowerCase()||'';for(const b of menu.querySelectorAll('[role=option]'))b.hidden=!`${b.textContent} ${b.dataset.group}`.toLowerCase().includes(query);for(const g of menu.querySelectorAll('.dropdown-group'))g.hidden=![...menu.querySelectorAll('[role=option]')].some(b=>!b.hidden&&b.dataset.group===g.dataset.group);}
 trigger.onclick=()=>menu.hidden?open():close();
 let typed='',lastTyped=0;
 function keydown(e){
  if(e.key==='Escape'&&!menu.hidden){e.preventDefault();e.stopPropagation();close();trigger.focus({preventScroll:true});return;}
  if(e.key==='Tab'&&!menu.hidden){close();trigger.focus({preventScroll:true});return;}
  if(['ArrowDown','ArrowUp','Home','End'].includes(e.key)){
   e.preventDefault();e.stopPropagation();if(menu.hidden){open();return;}const all=optionNodes(),idx=all.indexOf(document.activeElement);
   focusOption(all[e.key==='Home'?0:e.key==='End'?all.length-1:(idx+(e.key==='ArrowDown'?1:-1)+all.length)%all.length]);
  }else if(!menu.hidden&&e.target!==search&&e.key.length===1&&!e.metaKey&&!e.ctrlKey&&!e.altKey&&e.key!==' '){
   e.stopPropagation();const now=performance.now();typed=(now-lastTyped>600?'':typed)+e.key.toLowerCase();lastTyped=now;
   focusOption(optionNodes().find(option=>option.textContent.toLowerCase().startsWith(typed)));
  }
 }
 trigger.onkeydown=menu.onkeydown=keydown;
 const control={root,menu,close};
 if(activeDropdown&&container.contains(activeDropdown.root))activeDropdown.close();
 root.append(trigger,menu);container.replaceChildren(root);set(value);return {set,close};
}
document.addEventListener('pointerdown',e=>{if(activeDropdown&&!activeDropdown.root.contains(e.target)&&!activeDropdown.menu.contains(e.target))activeDropdown.close();});

function makeKnob(id,voice){
 const d=defs[id],parameter=voice*39+id;const root=el('div','knob');root.tabIndex=0;root.setAttribute('role','slider');root.setAttribute('aria-label',d.name);root.setAttribute('aria-valuemin',d.min);root.setAttribute('aria-valuemax',d.max);root.title=`${d.name} · Drag vertically · Shift for fine control · Enter to type · Double-click to reset`;
 root.innerHTML='<svg viewBox="0 0 64 64" aria-hidden="true"><path class="knob-track" d="M 14.32 49.68 A 25 25 0 1 1 49.68 49.68"/><path class="knob-value" d="M 14.32 49.68 A 25 25 0 1 1 49.68 49.68" pathLength="100"/><circle class="knob-body" cx="32" cy="32" r="18"/><line class="knob-line" x1="32" y1="18" x2="32" y2="25"/></svg>';
 const out=el('output'),label=el('span','knob-label',d.name);root.append(out,label);
 const arc=root.querySelector('.knob-value'),line=root.querySelector('.knob-line');
 root.onclick=e=>{if(e.target.tagName!=='INPUT')root.focus({preventScroll:true});};
 function update(){const v=effective(state,'params')[voice][id],norm=clamp(normalize(d,v),0,1);out.textContent=format(d,v);root.setAttribute('aria-valuenow',v);root.setAttribute('aria-valuetext',format(d,v));arc.style.strokeDasharray=`${norm*100} 100`;line.setAttribute('transform',`rotate(${-135+norm*270} 32 32)`);}
 function set(v){v=constrain(d,v);if(v===effective(state,'params')[voice][id])return;effective(state,'params')[voice][id]=v;command('lxr_param',voice,id,v);update();saveSoon();}
 const gesture=active=>{localEdits[active?'begin':'end'](parameter);native?.gesture(parameter,active);root.classList.toggle('dragging',active);};
 root.onpointerdown=e=>{
  if(e.button!==0 || e.target.tagName==='INPUT'||root.hasPointerCapture(e.pointerId))return;e.preventDefault();remember();root.focus({preventScroll:true});root.setPointerCapture(e.pointerId);
  const drag=new KnobDrag(normalize(d,effective(state,'params')[voice][id]),e.clientY);gesture(true);
  root.onpointermove=event=>{if(event.pointerId===e.pointerId)set(denormalize(d,drag.move(event.clientY,event.shiftKey)));};
  const finish=event=>{if(event?.pointerId!==undefined&&event.pointerId!==e.pointerId)return;root.onpointermove=root.onpointerup=root.onpointercancel=root.onlostpointercapture=null;removeEventListener('blur',finish);gesture(false);if(root.hasPointerCapture(e.pointerId))root.releasePointerCapture(e.pointerId);};
  root.onpointerup=root.onpointercancel=root.onlostpointercapture=finish;addEventListener('blur',finish,{once:true});
 };
 let lastWheel=-Infinity,wheelRemainder=0;
 root.addEventListener('wheel',e=>{
  if(e.ctrlKey||!e.deltaY)return;e.preventDefault();const now=performance.now();if(now-lastWheel>300){remember();wheelRemainder=0;}lastWheel=now;
  const pixels=wheelPixels(e)/(e.shiftKey?10:1);
  if(d.options||d.unit==='integer'){wheelRemainder-=pixels/80;const steps=Math.trunc(wheelRemainder);if(steps){wheelRemainder-=steps;set(effective(state,'params')[voice][id]+steps);}}
  else set(denormalize(d,normalize(d,effective(state,'params')[voice][id])-clamp(pixels/8000,-.04,.04)));
 },{passive:false});
 root.ondblclick=e=>{e.preventDefault();remember();const value=factoryKits[state.kit][voice][id];set(value);toast(`${d.name} reset`);};
 root.onkeydown=e=>{
  if(e.target.tagName==='INPUT')return;
  if(['ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Home','End'].includes(e.key)){
   e.preventDefault();remember();const sign=e.key==='ArrowUp'||e.key==='ArrowRight'?1:-1;
   if(e.key==='Home')set(d.min);else if(e.key==='End')set(d.max);else if(d.options||d.unit==='integer')set(effective(state,'params')[voice][id]+sign);else set(denormalize(d,normalize(d,effective(state,'params')[voice][id])+sign*(e.shiftKey?0.001:0.01)));
  }
  if(e.key==='Enter'&&!d.options){e.preventDefault();const input=el('input');input.type='number';input.value=Number(effective(state,'params')[voice][id].toPrecision(5));input.step='any';input.min=d.min;input.max=d.max;input.setAttribute('aria-label',`${d.name} value`);remember();root.append(input);input.focus();input.select();const finish=apply=>{if(apply&&Number.isFinite(input.valueAsNumber))set(input.valueAsNumber);input.remove();root.focus();};input.onkeydown=event=>{event.stopPropagation();if(event.key==='Enter'){input.onblur=null;finish(true);}if(event.key==='Escape'){input.onblur=null;finish(false);}};input.onblur=()=>finish(true);}
 };
 knobUpdates.push(update);update();return root;
}
function renderKnobs(){
 knobUpdates=[];
 const drumPages=[[0,2,5,8,7,1,11,12],[0,1,26,32,7,9,11,27],[3,2,4,28,5,6,37,19],[7,8,9,32,5,6,37,26]];
 const percussionPages=[[0,2,8,25,7,24,11,12],[0,1,26,9,38,34,11,27],[3,2,4,28,19,18,31,selectedTrack===4?34:29],[7,8,9,24,25,38,0,1]];
 const snarePages=[[0,2,1,10,35,12,11,27],[0,1,26,10,35,11,27,34],[3,2,4,28,5,6,37,19],[0,1,10,35,5,6,37,26]];
 let ids=page<4?(selectedTrack<3?drumPages:selectedTrack===3?snarePages:percussionPages)[page]:pages[page].params;
 ids=ids.map(id=>selectedTrack===6&&id===2?29:id);
 $('#knobs').replaceChildren(...ids.map(id=>makeKnob(id,Math.min(selectedTrack,5))));
 $$('#tabs button').forEach((b,i)=>{b.textContent=selectedTrack===3&&i===3?'NOISE':pages[i].name;b.classList.toggle('active',i===page);b.setAttribute('aria-current',String(i===page));});
}
function selectTrack(t){
 selectedTrack=(t+7)%7;$('#voice-number').textContent=String(selectedTrack+1).padStart(2,'0');$('#voice-name').textContent=tracks[selectedTrack];
 $('#voice-type').textContent=selectedTrack<3?'FM PERCUSSION':selectedTrack===3?'TONAL / NOISE':selectedTrack===4?'CLAP / METALLIC':'SHARED / CHOKE GROUP';
 $('#trigger kbd').textContent=keys[selectedTrack];
 $$('.channel').forEach((c,i)=>c.classList.toggle('selected',i===selectedTrack));
 $$('.track-label').forEach((b,i)=>b.classList.toggle('selected',i===selectedTrack));
 $('#track-length').value=state.patterns[state.pattern].lengths[selectedTrack];updateTimingControls();renderKnobs();features?.render();
}
function renderChannels(){
 const channels=tracks.map((name,t)=>{
  const c=el('div','channel');c.dataset.track=t;
  const label=el('button','channel-label',shortTracks[t]);label.setAttribute('aria-label',`Edit ${name}`);label.onclick=()=>selectTrack(t);
  const fader=el('div','channel-fader'),input=el('input');input.type='range';input.min=0;input.max=1.25;input.step=0.01;input.value=effective(state,'levels')[t];input.setAttribute('aria-label',`${name} level`);input.setAttribute('aria-orientation','vertical');input.onpointerdown=remember;input.oninput=()=>{effective(state,'levels')[t]=+input.value;value.textContent=format(defs[19],+input.value);command('lxr_level',t,+input.value);saveSoon();};
  const vu=el('div','vu');vu.append(el('i'));fader.append(input,vu);const value=el('output','channel-value',format(defs[19],effective(state,'levels')[t]));
  const buttons=el('div','channel-buttons');
  for(const type of ['mute','solo']){const b=el('button',type,type==='mute'?'M':'S');const field=type==='mute'?'muted':'solo';b.setAttribute('aria-label',`${type==='mute'?'Mute':'Solo'} ${name}`);b.setAttribute('aria-pressed',state[field][t]);b.classList.toggle('active',state[field][t]);b.onclick=()=>{remember();state[field][t]=!state[field][t];command(`lxr_${type}`,t,+state[field][t]);b.classList.toggle('active',state[field][t]);b.setAttribute('aria-pressed',state[field][t]);saveSoon();};buttons.append(b);}
  const pad=el('button','channel-pad');pad.innerHTML=`<i></i><span>${keys[t]}</span>`;pad.setAttribute('aria-label',`Trigger ${name}`);pad.title=`Trigger ${name} · ${keys[t]}`;pad.onpointerdown=e=>{e.preventDefault();selectTrack(t);trigger(t);};pad.onclick=e=>{if(e.detail===0){selectTrack(t);trigger(t);}};
  c.append(label,fader,value,buttons,pad);return c;
 });$('#channels').replaceChildren(...channels);
}
function syncStep(p,t,s){const a=state.patterns[p].tracks[t][s];command('lxr_set_step',p,t,s,a.v,a.prob,a.note,a.ratchet);command('lxr_step_extras',p,t,s,a.micro,a.condition,a.every,a.phase);for(let slot=0;slot<2;slot++)command('lxr_set_lock',p,t,s,slot,...a.locks[slot]);saveSoon();}
function updateStepButton(b,t,s){
 const a=state.patterns[state.pattern].tracks[t][s];b.classList.toggle('on',a.v>0);b.classList.toggle('inactive',s>=state.patterns[state.pattern].lengths[t]);b.classList.toggle('chosen',editedStep?.t===t&&editedStep?.s===s);
 b.setAttribute('aria-pressed',String(a.v>0));b.setAttribute('aria-label',`${tracks[t]}, step ${s+1}, ${a.v>0?'on':'off'}`);b.title=`${tracks[t]} · step ${s+1} · velocity ${Math.round(a.v*127)} · ${Math.round(a.prob*100)}% · right-click to edit`;
 const velocity=el('i','step-velocity');velocity.style.transform=`scaleX(${a.v})`;b.replaceChildren(velocity);
 if(a.locks.some(l=>l[0]!==255))b.append(el('i','lock-dot'));
 if(a.prob<0.999&&a.v>0)b.append(el('span','probability-mark','·'));
}
function renderGrid(){
 const grid=$('#sequence-grid'),nodes=[el('div','grid-corner','VOICE / STEP')];
 for(let i=0;i<16;i++)nodes.push(el('div',`step-number${i%4===0?' downbeat':''}`,String(bar*16+i+1).padStart(2,'0')));
 for(let t=0;t<7;t++){
  const label=el('button','track-label');label.innerHTML=`<span class="track-index">${String(t+1).padStart(2,'0')}</span><span>${tracks[t]}</span><span class="track-length">${divisions[state.patterns[state.pattern].divisions[t]][0]} · ${state.patterns[state.pattern].lengths[t]}</span>`;label.onclick=()=>selectTrack(t);label.setAttribute('aria-label',`Select ${tracks[t]}`);nodes.push(label);
  for(let i=0;i<16;i++){
   const s=bar*16+i,b=el('button',`step${i%4===0?' downbeat':''}`);b.dataset.track=t;b.dataset.step=s;updateStepButton(b,t,s);
   b.onclick=e=>{if(suppressStepClick){suppressStepClick=false;return;}if(e.altKey){openStep(t,s);return;}remember();const a=state.patterns[state.pattern].tracks[t][s];if(e.shiftKey)a.v=a.v===1?0.6:1;else a.v=a.v>0?0:0.85;syncStep(state.pattern,t,s);updateStepButton(b,t,s);};
   b.oncontextmenu=e=>{e.preventDefault();openStep(t,s);};let longTimer,startX,startY;
   b.onpointerdown=e=>{if(e.pointerType==='mouse')return;startX=e.clientX;startY=e.clientY;longTimer=setTimeout(()=>{suppressStepClick=true;openStep(t,s);},450);};b.onpointermove=e=>{if(Math.hypot(e.clientX-startX,e.clientY-startY)>8)clearTimeout(longTimer);};b.onpointerup=b.onpointercancel=()=>clearTimeout(longTimer);
   nodes.push(b);
  }
 }
 grid.replaceChildren(...nodes);$$('.track-label').forEach((b,t)=>b.classList.toggle('selected',t===selectedTrack));$('#track-length').value=state.patterns[state.pattern].lengths[selectedTrack];updateTimingControls();updatePlayhead();
}
function updatePlayhead(){
 $$('.step').forEach(b=>b.classList.toggle('playhead',playing&&audioPattern===state.pattern&&positions[+b.dataset.track]===+b.dataset.step));
 $$('#patterns button').forEach((b,i)=>{b.classList.toggle('selected',i===state.pattern);b.classList.toggle('queued',playing&&i===state.pattern&&audioPattern!==state.pattern);b.setAttribute('aria-pressed',i===state.pattern);});
}
function updateTimingControls(){
 const p=state.patterns[state.pattern],t=selectedTrack;divisionSelect?.set(p.divisions[t]);$('#track-hits').max=p.lengths[t];$('#track-hits').value=p.tracks[t].slice(0,p.lengths[t]).filter(a=>a.v>0).length;
}
function renderPatterns(){
 const items=Array.from({length:8},(_,p)=>{const b=el('button','',String(p+1).padStart(2,'0'));b.setAttribute('aria-label',`Pattern A${String(p+1).padStart(2,'0')}`);b.onclick=()=>selectPattern(p);return b;});$('#patterns').replaceChildren(...items);
 $('#pattern-label').textContent=`A${String(state.pattern+1).padStart(2,'0')}`;
 $('#bars').replaceChildren(...Array.from({length:4},(_,i)=>{const b=el('button','',String(i+1));b.classList.toggle('active',i===bar);b.setAttribute('aria-label',`Bar ${i+1}`);b.onclick=()=>{bar=i;renderGrid();$$('#bars button').forEach((n,j)=>n.classList.toggle('active',i===j));};return b;}));
}

function sliderField(label,value,min,max,step,onInput,formatter=x=>String(x)){
 const root=el('label','field');const top=el('span','field-label'),out=el('output','',formatter(value));top.append(el('span','',label),out);const input=el('input');input.type='range';input.min=min;input.max=max;input.step=step;input.value=value;input.setAttribute('aria-label',label);input.oninput=()=>{out.textContent=formatter(+input.value);onInput(+input.value);};root.append(top,input);return root;
}
function openStep(t,s){
 selectTrack(t);editedStep={p:state.pattern,t,s};remember();const a=state.patterns[state.pattern].tracks[t][s];$('#step-title').textContent=`${tracks[t]} · ${String(s+1).padStart(2,'0')}`;
 const update=()=>{syncStep(editedStep.p,t,s);const button=$(`.step[data-track="${t}"][data-step="${s}"]`);if(button)updateStepButton(button,t,s);};
 $('#step-fields').replaceChildren(sliderField('Velocity',Math.round(a.v*127),0,127,1,v=>{a.v=v/127;update();}),sliderField('Probability',Math.round(a.prob*100),0,100,1,v=>{a.prob=v/100;update();},v=>`${v}%`),sliderField('Note',a.note,-36,36,1,v=>{a.note=v;update();},v=>`${v>0?'+':''}${v} st`),sliderField('Ratchet',a.ratchet,1,4,1,v=>{a.ratchet=v;update();},v=>`${v}×`));
 $('#step-fields').append(sliderField('Microtiming',a.micro*100,-49,49,1,v=>{a.micro=v/100;update();},v=>`${v>0?'+':''}${Math.round(v)}%`));
 const condition=el('div','feature-field'),conditionLabel=el('span','micro-label','Condition'),conditionSelect=el('div');condition.append(conditionLabel,conditionSelect);
 selectWidget(conditionSelect,conditionNames.map((label,value)=>({label,value})),a.condition,value=>{a.condition=value;update();$('#step-cycle').hidden=value!==7;},'Step condition');$('#step-fields').append(condition);
 const cycle=el('div','step-cycle');cycle.id='step-cycle';cycle.hidden=a.condition!==7;
 for(const [label,key,min,max] of [['Every','every',1,16],['Pass','phase',1,16]]){const row=el('label','feature-field'),input=el('input');input.type='number';input.min=min;input.max=max;input.value=key==='phase'?a.phase+1:a.every;input.setAttribute('aria-label',label==='Every'?'Condition cycle length':'Condition pass');input.oninput=()=>{if(!input.value)return;a[key]=clamp(Math.round(+input.value||1),min,max)-(key==='phase'?1:0);a.phase%=a.every;cycle.querySelectorAll('input')[1].value=a.phase+1;update();};row.append(el('span','micro-label',label),input);cycle.append(row);}$('#step-fields').append(cycle);
 const lockRows=[0,1].map(slot=>{
  const row=el('div','lock-row'),select=el('div'),value=el('div','lock-value'),input=el('input'),out=el('output');input.type='range';input.min=0;input.max=1;input.step=0.001;input.setAttribute('aria-label',`Lock ${slot+1} value`);value.append(input,out);
  function updateValue(){const id=a.locks[slot][0];input.disabled=id===255;if(id===255){input.value=0;out.textContent='—';}else{input.value=normalize(defs[id],a.locks[slot][1]);out.textContent=format(defs[id],a.locks[slot][1]);}}
  selectWidget(select,[{value:255,label:'No lock'},...defs.map(d=>({value:d.id,label:d.name}))],a.locks[slot][0],id=>{a.locks[slot]=id===255?[255,0]:[id,effective(state,'params')[Math.min(t,5)][id]];updateValue();update();},`Lock ${slot+1} parameter`);
  input.oninput=()=>{const id=a.locks[slot][0];if(id===255)return;a.locks[slot][1]=denormalize(defs[id],+input.value);out.textContent=format(defs[id],a.locks[slot][1]);update();};updateValue();row.append(select,value);return row;
 });$('#step-locks').replaceChildren(...lockRows);$('#step-dialog').showModal();
}
function generatorPreview(){
 const n=clamp(Math.round(+$('#gen-length').value||1),1,64),k=clamp(Math.round(+$('#gen-hits').value||0),0,n),r=clamp(Math.round(+$('#gen-rotation').value||0),0,63);
 $('#gen-hits').max=n;$('#gen-hits').value=k;$('#generator-preview').replaceChildren(...Array.from({length:n},(_,i)=>el('i',((i+n-r%n)%n*k)%n<k?'on':'')));$('#generator-summary').textContent=`${k} hits in ${n} steps`;return {n,k,r};
}
function delayTimeLimits(){
 const r=effective(state,'fx')[14],max=effective(state,'fx')[0]===5?15728.159:32767;
 const low=r*882.8+(1-r)*26.484,high=r*max+(1-r)*882.8;
 return [low/44140,(low+r*(high-low))/44140];
}
function boundFxTime(){
 if(effective(state,'fx')[0]!==4&&effective(state,'fx')[0]!==5)return;
 const [min,max]=delayTimeLimits(),value=clamp(effective(state,'fx')[2],min,max);
 if(value!==effective(state,'fx')[2]){effective(state,'fx')[2]=value;command('lxr_fx',2,value);saveSoon();}
}
function renderFx(){
 boundFxTime();
 const kind=effective(state,'fx')[0],pct=v=>`${Math.round(v*100)}%`;
 const time=v=>v<0.01?`${(v*1000).toFixed(1)} ms`:`${Math.round(v*1000)} ms`;
 const configs=[1,6,7].includes(kind)?[[5,'DRIVE',pct],[7,kind===1?'BIAS':kind===6?'OFFSET':'FEEDBACK',pct],[4,'TONE',pct],[8,'LEVEL',pct],[1,'MIX',pct]]:kind===2?[[2,'FREQUENCY',v=>`${Math.round(v/0.7*880)} Hz`,0,0.7],[9,'WAVEFORM',v=>defs[1].options[Math.round(v)],0,5,1],[1,'MIX',pct]]:kind===3?[[10,'ATTACK',v=>time((v*100+0.03)/1000)],[11,'RELEASE',v=>time((v*175+25)/1000)],[12,'THRESHOLD',v=>`${((v-1)*96).toFixed(1)} dB`],[13,'RATIO',v=>`${Math.round(v)}:1`,1,8,1],[5,'MAKEUP',v=>`${(20*Math.log10(1+v*2)).toFixed(1)} dB`],[1,'MIX',pct]]:[[2,'TIME',time,...delayTimeLimits()],[14,'RANGE',pct],[3,'FEEDBACK',pct,0,0.94],[4,'TONE',pct],[1,'MIX',pct]];
 $('#fx-controls').classList.toggle('disabled',kind===0);
 $('#fx-controls').replaceChildren(...configs.map(([id,label,formatter,min=0,max=1,step=0.001])=>{
  const root=el('label','fx-control'),top=el('div'),out=el('output','',formatter(effective(state,'fx')[id])),input=el('input');top.append(el('span','',label),out);input.type='range';input.min=min;input.max=max;input.step=step;input.value=effective(state,'fx')[id];input.disabled=kind===0;input.setAttribute('aria-label',`FX ${label.toLowerCase()}`);input.onpointerdown=remember;input.oninput=()=>{effective(state,'fx')[id]=+input.value;command('lxr_fx',id,+input.value);out.textContent=formatter(+input.value);saveSoon();};root.dataset.fxId=id;root.formatValue=formatter;root.append(top,input);if(id===14){const previous=input.oninput;input.oninput=()=>{previous();boundFxTime();const field=$('[data-fx-id=\"2\"]');const [low,high]=delayTimeLimits();field.querySelector('input').min=low;field.querySelector('input').max=high;field.querySelector('input').value=effective(state,'fx')[2];field.querySelector('output').textContent=time(effective(state,'fx')[2]);};}return root;
 }));
}
function renderAll(){
 $('#bpm').value=Math.round(state.bpm);$('#swing').value=state.swing;$('#swing-value').textContent=`${Math.round(state.swing*100)}%`;$('#master').value=effective(state,'fx')[6];$('#master-value').textContent=format(defs[19],effective(state,'fx')[6]);$('#morph').value=effective(state,'morph');$('#morph-value').textContent=`${Math.round(effective(state,'morph')*100)}%`;$('#morph-target').textContent=`→ ${kits[effective(state,'morphTarget')]}`;$('#session-name').value=state.name;
 kitSelect?.set(state.kit);fxSelect?.set(effective(state,'fx')[0]);renderChannels();renderPatterns();renderGrid();renderFx();selectTrack(selectedTrack);
}

// Host automation updates existing controls, preserving pointer capture and focus.
function refreshNativeControls(fxChanged){
 for(const update of knobUpdates)update();features?.refresh();
 $$('.channel').forEach((channel,t)=>{
  channel.querySelector('input').value=effective(state,'levels')[t];channel.querySelector('.channel-value').textContent=format(defs[19],effective(state,'levels')[t]);
  for(const [type,field] of [['mute','muted'],['solo','solo']]){const button=channel.querySelector('.'+type);button.classList.toggle('active',state[field][t]);button.setAttribute('aria-pressed',state[field][t]);}
 });
 $('#swing').value=state.swing;$('#swing-value').textContent=`${Math.round(state.swing*100)}%`;
 $('#master').value=effective(state,'fx')[6];$('#master-value').textContent=format(defs[19],effective(state,'fx')[6]);
 $('#morph').value=effective(state,'morph');$('#morph-value').textContent=`${Math.round(effective(state,'morph')*100)}%`;$('#morph-target').textContent=`→ ${kits[effective(state,'morphTarget')]}`;
 if(fxChanged){fxSelect?.set(effective(state,'fx')[0]);renderFx();}
 else $$('[data-fx-id]').forEach(control=>{const id=+control.dataset.fxId,input=control.querySelector('input');if(id===2&&(effective(state,'fx')[0]===4||effective(state,'fx')[0]===5))[input.min,input.max]=delayTimeLimits();input.value=effective(state,'fx')[id];control.querySelector('output').textContent=control.formatValue(effective(state,'fx')[id]);});
}

async function ensureAudio(){
 if(native)return;
 if(audio){await audio.context.resume();return audio;}if(audioPending)return audioPending;
 audioPending=(async()=>{
  if(!window.AudioContext||!window.AudioWorkletNode)throw new Error('AudioWorklet is unavailable. Open this instrument in a current browser on localhost.');
  $('#audio-status span').textContent='Starting…';const context=new AudioContext({sampleRate:44100,latencyHint:'interactive'});await context.resume();
  try {
   await context.audioWorklet.addModule('/audio-worklet.js');
   const node=new AudioWorkletNode(context,'lxr02',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{module}});
   const analyser=context.createAnalyser();analyser.fftSize=2048;node.connect(analyser);analyser.connect(context.destination);
   await new Promise((resolve,reject)=>{
    const timeout=setTimeout(()=>reject(new Error('Audio engine did not start.')),8000);
    node.port.onmessage=({data})=>{
     if(data.error){clearTimeout(timeout);reject(new Error(data.error));setPlaying(false);$('#audio-status').className='audio-status error';report(data.error);return;}
     if(data.ready){clearTimeout(timeout);resolve();return;}
     if(data.positions){positions=data.positions;audioPattern=data.pattern;updatePlayhead();$$('.vu i').forEach((n,t)=>n.style.height=`${Math.min(100,Math.sqrt(data.meters[t])*100)}%`);$('#meter-l').style.width=`${Math.min(100,data.peak[0]*100)}%`;$('#meter-r').style.width=`${Math.min(100,data.peak[1]*100)}%`;}
    };
   });
   audio={context,node,analyser};node.port.postMessage({batch:snapshotCommands(state),samples:state.samples});
   $('#audio-status').className='audio-status ready';$('#audio-status span').textContent='Audio on';$('#engine-info').textContent=`${(context.sampleRate/1000).toFixed(1)} kHz / 128 samples / stereo`;
   context.onstatechange=()=>{if(context.state==='suspended'){$('#audio-status span').textContent='Resume audio';$('#audio-status').classList.remove('ready');}else if(context.state==='running'){$('#audio-status span').textContent='Audio on';$('#audio-status').classList.add('ready');}};
   return audio;
  }catch(error){await context.close();throw error;}
 })();
 try{return await audioPending;}finally{audioPending=null;}
}
function setPlaying(value){playing=value;$('#play').classList.toggle('running',value);$('#play').setAttribute('aria-label',value?'Stop':'Play');$('#play').setAttribute('aria-pressed',String(value));$('#play svg').innerHTML=value?'<path d="M5 5h10v10H5z" fill="currentColor"/>':'<path d="M6 3.5 16 10 6 16.5Z" fill="currentColor"/>';updatePlayhead();}
async function toggleTransport(){if(native)return;try{if(playing){command('lxr_stop');positions.fill(-1);setPlaying(false);}else{await ensureAudio();audioPattern=state.pattern;command('lxr_pattern',state.pattern);command('lxr_play');setPlaying(true);}}catch(error){$('#audio-status span').textContent='Audio error';report(error);}}
async function trigger(t){try{await ensureAudio();command('lxr_trigger',t,0.95,0);const pad=$(`.channel[data-track="${t}"] .channel-pad`);pad?.classList.add('pressed');if(t===selectedTrack)$('#trigger').classList.add('pressed');setTimeout(()=>{pad?.classList.remove('pressed');$('#trigger').classList.remove('pressed');},100);}catch(error){report(error);}}

function download(blob,name){const a=el('a');a.href=URL.createObjectURL(blob);a.download=name;document.body.append(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(a.href),10000);}
function filename(){return (state.name||'lxr02').replace(/[^\p{L}\p{N}_-]+/gu,'-').toLowerCase().slice(0,60)||'lxr02';}
function bindActions(){
 $('#feature-body').addEventListener('pointerdown',e=>{if(e.target.matches('input[type=range]'))remember();});
 $('#play').onclick=toggleTransport;$('#stop').onclick=()=>{command('lxr_stop');positions.fill(-1);setPlaying(false);};$('#audio-status').onclick=()=>ensureAudio().catch(report);$('#trigger').onpointerdown=e=>{e.preventDefault();trigger(selectedTrack);};$('#trigger').onclick=e=>{if(e.detail===0)trigger(selectedTrack);};
 $('#previous-voice').onclick=()=>selectTrack(selectedTrack-1);$('#next-voice').onclick=()=>selectTrack(selectedTrack+1);
 $('#bpm').onfocus=remember;$('#bpm').onchange=()=>{state.bpm=clamp(+$('#bpm').value||128,30,300);$('#bpm').value=state.bpm;command('lxr_tempo',state.bpm,state.swing);saveSoon();};
 $('#tap').onclick=()=>{const now=performance.now();if(tapTimes.length&&now-tapTimes.at(-1)>2000)tapTimes=[];tapTimes.push(now);tapTimes=tapTimes.slice(-5);if(tapTimes.length>=2){remember();state.bpm=clamp(Math.round(60000/((now-tapTimes[0])/(tapTimes.length-1))),30,300);$('#bpm').value=state.bpm;command('lxr_tempo',state.bpm,state.swing);saveSoon();}};
 $('#swing').onpointerdown=remember;$('#swing').oninput=()=>{state.swing=+$('#swing').value;$('#swing-value').textContent=`${Math.round(state.swing*100)}%`;command('lxr_tempo',state.bpm,state.swing);saveSoon();};
 $('#master').onpointerdown=remember;$('#master').oninput=()=>{effective(state,'fx')[6]=+$('#master').value;$('#master-value').textContent=format(defs[19],effective(state,'fx')[6]);command('lxr_fx',6,effective(state,'fx')[6]);saveSoon();};
 $('#morph').onpointerdown=remember;$('#morph').oninput=()=>{state.morph=+$('#morph').value;updateScene(state,'morph',state.morph);$('#morph-value').textContent=`${Math.round(state.morph*100)}%`;command('lxr_morph',state.morph,state.morphTarget);saveSoon();};
 $('#morph-target').onclick=()=>{remember();state.morphTarget=(effective(state,'morphTarget')+1)%5;updateScene(state,'morphTarget',state.morphTarget);$('#morph-target').textContent=`→ ${kits[state.morphTarget]}`;command('lxr_morph',state.morph,state.morphTarget);saveSoon();};
 $('#track-length').onblur=updateTimingControls;$('#track-length').onfocus=remember;$('#track-length').oninput=()=>{if(!$('#track-length').value)return;const n=clamp(Math.round(+$('#track-length').value||16),1,64);state.patterns[state.pattern].lengths[selectedTrack]=n;command('lxr_length',state.pattern,selectedTrack,n);renderGrid();features.render();saveSoon();};
 divisionSelect=selectWidget($('#track-division'),divisions.map(([label],value)=>({label,value})),4,value=>{remember();const p=state.patterns[state.pattern];p.divisions[selectedTrack]=value;command('lxr_track_settings',state.pattern,selectedTrack,value,p.delays[selectedTrack],p.grooves[selectedTrack],p.grooveAmounts[selectedTrack]);renderGrid();features.render();saveSoon();},'Selected track timing');
 $('#track-hits').onblur=updateTimingControls;$('#track-hits').onfocus=remember;$('#track-hits').oninput=()=>{if(!$('#track-hits').value)return;const p=state.patterns[state.pattern],n=p.lengths[selectedTrack],k=clamp(Math.round(+$('#track-hits').value||0),0,n),changes=[];for(let i=0;i<n;i++){const a=p.tracks[selectedTrack][i];a.v=(i*k)%n<k?(a.v||.85):0;changes.push(['lxr_set_step',[state.pattern,selectedTrack,i,a.v,a.prob,a.note,a.ratchet]]);}batch(changes);renderGrid();features.render();saveSoon();};
 $('#copy-pattern').onclick=()=>{remember();const to=(state.pattern+1)%8;state.patterns[to]=structuredClone(state.patterns[state.pattern]);state.scenes[to]=structuredClone(state.scenes[state.pattern]);publishProject();toast(`Copied to A${String(to+1).padStart(2,'0')}`);};
 $('#clear-track').onclick=()=>{remember();for(let s=0;s<64;s++){const a=state.patterns[state.pattern].tracks[selectedTrack][s];a.v=0;a.locks=[[255,0],[255,0]];syncStep(state.pattern,selectedTrack,s);}renderGrid();toast(`${tracks[selectedTrack]} cleared`);};
 $('#undo').onclick=()=>{if(!past.length)return;const wasPlaying=playing;const previous=past.pop();state=JSON.parse(previous.json);state.samples.forEach((a,t)=>{if(a)a.data=previous.samples[t];});batch(snapshotCommands(state));if(wasPlaying)command('lxr_play');syncSamples();$('#undo').disabled=!past.length;renderAll();saveSoon();};
 $('#generate').onclick=()=>{$('#generator-title').textContent=tracks[selectedTrack];const track=state.patterns[state.pattern];$('#gen-length').value=track.lengths[selectedTrack];$('#gen-hits').value=track.tracks[selectedTrack].slice(0,track.lengths[selectedTrack]).filter(a=>a.v>0).length;$('#gen-rotation').value=0;generatorPreview();$('#generator-dialog').showModal();};
 for(const id of ['#gen-hits','#gen-length','#gen-rotation'])$(id).oninput=generatorPreview;
 $('#apply-generator').onclick=()=>{remember();const {n,k,r}=generatorPreview();command('lxr_generate',state.pattern,selectedTrack,k,n,r);state.patterns[state.pattern]=readPattern(state.pattern);renderGrid();features.render();saveSoon();$('#generator-dialog').close();};
 $('#step-remove').onclick=()=>{if(editedStep){const {p,t,s}=editedStep;state.patterns[p].tracks[t][s].v=0;syncStep(p,t,s);$('#step-dialog').close();renderGrid();}};
 $('#step-dialog').addEventListener('close',()=>{editedStep=null;renderGrid();});
 $('#step-help').onclick=()=>{const first=state.patterns[state.pattern].tracks[selectedTrack].findIndex(a=>a.v>0);openStep(selectedTrack,first>=0?first:bar*16);};
 $$('.close-dialog').forEach(b=>b.onclick=()=>b.closest('dialog').close());$$('dialog').forEach(d=>d.addEventListener('click',e=>{if(e.target===d){const r=d.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)d.close();}}));
 $('#project').onclick=()=>{$('#session-name').value=state.name;$('#session-dialog').showModal();};$('#session-name').onchange=()=>{state.name=$('#session-name').value.trim()||'Session 01';saveSoon();};
 $('#save-project').onclick=()=>{state.name=$('#session-name').value.trim()||'Session 01';download(new Blob([JSON.stringify(state,null,2)],{type:'application/json'}),`${filename()}.dxm088.json`);saveSoon();};
 $('#import-project').onclick=()=>$('#project-file').click();
 $('#project-file').onchange=async()=>{const file=$('#project-file').files[0];if(!file)return;try{if(file.size>32_000_000)throw new Error('Project file is too large.');const incoming=validateProject(JSON.parse(await file.text()));remember();command('lxr_panic');setPlaying(false);state=incoming;batch(snapshotCommands(state));syncSamples();renderAll();saveSoon();$('#session-dialog').close();toast('Project loaded');}catch(error){report(error);}finally{$('#project-file').value='';}};
 $('#export').onclick=()=>features.showExport();
 document.addEventListener('keydown',e=>{
  if(e.defaultPrevented||e.target.closest('input,textarea,[contenteditable=true]')||$('dialog[open]'))return;
  if((e.metaKey||e.ctrlKey)&&e.code==='KeyZ'){e.preventDefault();$('#undo').click();return;}
  if(e.code==='Space'){e.preventDefault();if(!e.repeat)toggleTransport();return;}
  if(e.ctrlKey||e.metaKey||e.altKey||e.repeat)return;
  const t=keys.indexOf(e.key.toUpperCase());if(t>=0){e.preventDefault();selectTrack(t);trigger(t);}
  if(e.key==='Escape'){command('lxr_panic');positions.fill(-1);setPlaying(false);}
 });
 window.addEventListener('pagehide',()=>{if(native)return;storeProject(state).catch(()=>{});});
}
function animate(){
 const canvas=$('#scope'),ctx=canvas.getContext('2d');ctx.clearRect(0,0,canvas.width,canvas.height);ctx.strokeStyle='#2c3528';ctx.lineWidth=1;ctx.beginPath();ctx.moveTo(0,35);ctx.lineTo(400,35);ctx.stroke();
 if(audio||native){if(audio)audio.analyser.getFloatTimeDomainData(waveform);ctx.strokeStyle='#eab460';ctx.lineWidth=1.4;ctx.beginPath();for(let i=0;i<400;i++){const value=waveform[Math.floor(i*waveform.length/400)];const y=35-value*31;if(i===0)ctx.moveTo(i,y);else ctx.lineTo(i,y);}ctx.stroke();}
 requestAnimationFrame(animate);
}
async function boot(){
 try {
  let wasmBytes;if(globalThis.lxrWasmBase64)wasmBytes=Uint8Array.from(atob(globalThis.lxrWasmBase64),c=>c.charCodeAt(0));else{const response=await fetch('/lxr02.wasm');if(!response.ok)throw new Error('WASM engine is missing. Run ./run.sh to build it.');wasmBytes=await response.arrayBuffer();}
  module=await WebAssembly.compile(wasmBytes);core=new WebAssembly.Instance(module,{}).exports;core.lxr_init(44100);bindDefinitions(core);factoryKits=kits.map((_,i)=>{core.lxr_load_kit(i);return readParams();});core.lxr_load_kit(0);state=freshState();
  try{const stored=native?null:await readStoredProject();if(stored){const parsed=JSON.parse(stored);if(parsed.version===1&&!localStorage.getItem('lxr02-project-v1-original'))localStorage.setItem('lxr02-project-v1-original',stored);state=validateProject(parsed);batch(snapshotCommands(state));syncSamples();if(parsed.version===1)saveSoon();}}catch(error){console.warn('Saved session could not be restored:',error);toast('Saved session unavailable · loaded a fresh session');}
  $('#tabs').replaceChildren(...pages.map((p,i)=>{const b=el('button','',p.name);b.onclick=()=>{page=i;renderKnobs();};return b;}));
  kitSelect=selectWidget($('#kit-select'),kits.map((label,value)=>({label:`${String(value+1).padStart(2,'0')} · ${label}`,value})),state.kit,index=>{remember();state.kit=index;state.morph=0;command('lxr_load_kit',index);state.params=readParams();updateScene(state,'params',state.params);renderAll();saveSoon();},'Drum kit');
  fxSelect=selectWidget($('#fx-type'),['Off','Tube drive','Ring mod','Compressor','Delay','Ping pong','Fold','Clip'].map((label,value)=>({label,value})),effective(state,'fx')[0],index=>{remember();effective(state,'fx')[0]=index;command('lxr_fx',0,index);renderFx();saveSoon();},'Effect type');
  features=createPerformanceUI({el,selectWidget,sliderField,state:()=>state,track:()=>selectedTrack,module:()=>module,snapshot:snapshotCommands,native,remember,command,save:saveSoon,publish:publishProject,syncSamples,renderAll,renderKnobs,selectPattern,toast,report,filename,download});
  bindActions();renderAll();if(native)await bindNative();$('#engine-info').textContent=native?(native.preview?'DAW editor preview':'MIDI gated · host sync'):'Instrument ready · press play';document.body.setAttribute('aria-busy','false');requestAnimationFrame(animate);
 }catch(error){$('#load-error').hidden=false;$('#load-error').textContent=error.message;$('#engine-info').textContent='Engine unavailable';document.body.setAttribute('aria-busy','false');console.error(error);}
}
boot();

// The native instance is the source of truth in a DAW. WASM is only an editor model.
async function bindNative(){
 addEventListener('lxr-native-saved',()=>toast('Project saved'));
 document.body.classList.add("native-plugin");
 const applyProject=project=>{if(!project)return;native.supportsFileDrag=!!project.plugin?.nativeFileDrag;receivingNative=true;try{state=validateProject(project);batch(snapshotCommands(state));renderAll();const base=project.plugin?.baseNote??24;syncSamples();$('#pattern-note-base').value=base;updateNoteRange(base);}finally{receivingNative=false;}};
 const label=document.createElement('label');label.className='native-note-control';label.innerHTML='<span>MIDI</span><input id="pattern-note-base" aria-label="Pattern base MIDI note" type="number" min="0" max="120" value="24"><output id="pattern-note-range">24–31</output>';
 $('#tap').replaceWith(label);
 const updateNoteRange=base=>{const names=['C','C♯','D','D♯','E','F','F♯','G','G♯','A','A♯','B'];const note=n=>names[n%12]+(Math.floor(n/12)-1);$('#pattern-note-range').textContent=`${note(base)}–${note(base+7)}`;label.title=`MIDI ${base}–${base+7} → A01–A08 · last held note`;};
 $('#pattern-note-base').onchange=e=>{const value=Math.round(clamp(+e.target.value||0,0,120));e.target.value=value;updateNoteRange(value);state.plugin={version:1,baseNote:value};native.parameter(273,value);};
 $('#bpm').readOnly=true;$('#bpm').title='DAW tempo';$('#play').disabled=true;$('#play').title='Hold a MIDI note to play its pattern';$('#export').hidden=false;
 $('#audio-status').classList.add('enabled');$('#audio-status span').textContent=native.preview?'Preview':'DAW audio';$('#audio-status').title='Audio is processed by the native Rust instrument';
 $('.keyboard-hints').textContent='HOLD NOTE → PATTERN';$('#session-dialog .secondary').textContent='The DAW project stores all kits, patterns and locks. Project files also open in the web instrument.';
 addEventListener('lxr-native-project',e=>{if(nativeBooted)applyProject(e.detail);});
 let refreshPending=false,fxChanged=false;
 addEventListener('lxr-native-parameter',({detail:{id,value:norm,physical=false}})=>{
  if(!nativeBooted)return;let value;
  if(id<234){const v=Math.floor(id/39),p=id%39,d=defs[p];value=constrain(d,physical?norm:d.min+norm*(d.max-d.min));if(!localEdits.accept(id,value,performance.now())||Math.abs(state.params[v][p]-value)<=Math.max(1e-6,Math.abs(value)*1e-6))return;state.params[v][p]=value;if(state.scenes[state.pattern]?.enabled)state.scenes[state.pattern].params[v][p]=value;core.lxr_param(v,p,value);}
  else if(id<249){const p=id-234,ranges=[[0,7],[0,1],[0,.743],[0,.94],[0,1],[0,1],[0,1.5],[0,1],[0,1],[0,5],[0,1],[0,1],[0,1],[1,8],[0,1]];value=physical?norm:ranges[p][0]+norm*(ranges[p][1]-ranges[p][0]);if([0,9,13].includes(p))value=Math.round(value);if(!localEdits.accept(id,value,performance.now())||Math.abs(state.fx[p]-value)<=1e-6)return;state.fx[p]=value;if(state.scenes[state.pattern]?.enabled)state.scenes[state.pattern].fx[p]=value;core.lxr_fx(p,value);}
  else if(id<270){const group=id<256?'levels':id<263?'muted':'solo',start=id<256?249:id<263?256:263;value=group==='levels'?(physical?norm:norm*1.25):norm>=.5;if(!localEdits.accept(id,+value,performance.now())||Math.abs(+state[group][id-start]-value)<=1e-6)return;state[group][id-start]=value;if(state.scenes[state.pattern]?.enabled)state.scenes[state.pattern][group][id-start]=value;core[group==='levels'?'lxr_level':group==='muted'?'lxr_mute':'lxr_solo'](id-start,+value);}
  else if(id===270){value=physical?norm:norm*.5;if(!localEdits.accept(id,value,performance.now()))return;state.swing=value;core.lxr_tempo(state.bpm,state.swing);}
  else if(id===271||id===272){value=id===271?norm:Math.round(physical?norm:norm*4);if(!localEdits.accept(id,value,performance.now()))return;state[id===271?'morph':'morphTarget']=value;updateScene(state,id===271?'morph':'morphTarget',value);core.lxr_morph(state.morph,state.morphTarget);}
  else if(id>=274&&id<278){value=norm;if(!localEdits.accept(id,value,performance.now())||Math.abs(state.performance.macros[id-274]-value)<1e-6)return;state.performance.macros[id-274]=value;const scene=state.scenes[state.pattern];if(scene?.enabled)scene.macros[id-274]=value;core.lxr_macro(id-274,value);}
  else if(id===278){state.performance.velocityDepth=norm;}
  else if(id===280){state.performance.switchQuant=Math.round(physical?norm:norm*2);}
  else if(id===281||id===282){state.performance.lfoPeriods[id-281]=physical?norm:.0625+norm*(32-.0625);}
  else if(id===273){value=Math.round(physical?norm:norm*120);state.plugin={version:1,baseNote:value};$('#pattern-note-base').value=value;updateNoteRange(value);}
  if(id===234)fxChanged=true;
  if(!refreshPending){refreshPending=true;requestAnimationFrame(()=>{refreshPending=false;refreshNativeControls(fxChanged);fxChanged=false;});}
 });
 addEventListener('lxr-native-transport',({detail:s})=>{
  if(!nativeBooted)return;state.bpm=s.bpm||state.bpm;$('#bpm').value=Number(state.bpm.toFixed(2));positions=s.positions.map(Math.round);audioPattern=Math.round(s.pattern);setPlaying(s.note>=0);
  if(s.note>=0&&state.pattern!==audioPattern){state.pattern=audioPattern;renderAll();}
  if(s.meters)$$('.vu i').forEach((n,t)=>n.style.height=`${Math.min(100,Math.sqrt(s.meters[t])*100)}%`);if(s.wave)for(let i=0;i<waveform.length;i++)waveform[i]=s.wave[Math.floor(i*s.wave.length/waveform.length)];
  $('#meter-l').style.width=`${Math.min(100,s.left*100)}%`;$('#meter-r').style.width=`${Math.min(100,s.right*100)}%`;
  $('#engine-info').textContent=s.note>=0?`MIDI ${Math.round(s.note)} · A${String(audioPattern+1).padStart(2,'0')} · ${s.running?'DAW sync':'live'}`:'MIDI gated · host sync';
 });
 updateNoteRange(24);const project=await native.ready();applyProject(project);nativeBooted=true;
}
