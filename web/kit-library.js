import {effective,defaultPerformance} from './feature-state.js';

const fields=['params','levels','muted','solo','fx','morph','morphTarget'];
export function captureKit(state,name,id=crypto.randomUUID()){
 const sound=Object.fromEntries(fields.map(key=>[key,structuredClone(effective(state,key))]));
 sound.performance=structuredClone(effective(state,'performance'));
 sound.samples=structuredClone(state.samples);
 return {format:'dxm088-kit',version:1,id,name:name.trim().slice(0,60),updatedAt:new Date().toISOString(),factoryKit:state.kit,sound};
}
// Use the project's existing sound/sample validator; patterns and MIDI routing
// come from the current project and never become part of a kit.
export function applyKit(state,kit,validate){
 if(kit?.format!=='dxm088-kit'||kit.version!==1||typeof kit.name!=='string'||!kit.name.trim()||typeof kit.id!=='string'||!/^[a-zA-Z0-9-]{1,64}$/.test(kit.id)||!kit.sound)throw new Error('This kit could not be loaded.');
 const incoming={...state,...Object.fromEntries(fields.map(key=>[key,kit.sound[key]])),samples:kit.sound.samples,kit:kit.factoryKit??0,kitPreset:{id:kit.id,name:kit.name.slice(0,60)},performance:{...defaultPerformance(),...kit.sound.performance,switchQuant:state.performance.switchQuant}};
 // Loading a kit also updates the sound of the selected scene. Other scenes
 // retain their own sounds, and every pattern retains its rhythm.
 incoming.scenes=state.scenes.map((scene,p)=>p===state.pattern&&scene?.enabled?{...scene,...Object.fromEntries(fields.map(key=>[key,kit.sound[key]])),macros:kit.sound.performance?.macros??[0,0,0,0]}:scene);
 return validate(incoming);
}

let databasePromise;
function database(){return databasePromise??=new Promise((resolve,reject)=>{const request=indexedDB.open('dxm088-kits',1);request.onupgradeneeded=()=>request.result.createObjectStore('kits',{keyPath:'id'});request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});}
export function createKitLibrary(native){
 const request=async(op,kit,id)=>{
  if(native&&!native.preview)return native.kits(op,{kit,id});
  const db=await database();
  return new Promise((resolve,reject)=>{
   const tx=db.transaction('kits',op==='save'?'readwrite':'readonly'),store=tx.objectStore('kits');let result;
   const req=op==='list'?store.getAll():op==='get'?store.get(id):store.put(kit);
   req.onsuccess=()=>{result=req.result;};tx.oncomplete=()=>resolve(op==='list'?result.map(({id,name,updatedAt})=>({id,name,updatedAt})):op==='save'?kit:result);tx.onerror=()=>reject(tx.error);tx.onabort=()=>reject(tx.error);
  });
 };
 return {list:()=>request('list'),get:id=>request('get',undefined,id),save:kit=>request('save',kit)};
}
