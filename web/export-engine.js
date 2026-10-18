import {uploadSamples} from './feature-state.js';
const sleep=()=>new Promise(r=>setTimeout(r,0));
export async function renderPattern(module,commands,state,{beats=16,track=null,audio=true,onProgress=()=>{}}={}){
 const e=new WebAssembly.Instance(module,{}).exports,sr=44100;e.lxr_init(sr);
 for(const [fn,args] of commands)e[fn](...args);uploadSamples(e,state.samples);
 if(track!==null){for(let t=0;t<7;t++){e.lxr_solo(t,t===track?1:0);for(let p=0;p<8;p++)if(state.scenes[p])e.lxr_scene_mix(p,t,2,t===track?1:0);}}
 const music=Math.round(beats*60/state.bpm*sr),frames=music+(audio?sr*2:0),pcm=audio?new Float32Array(frames*2):null,hits=[];
 e.lxr_events_clear();e.lxr_play();let offset=0,blocks=0,view;
 while(offset<frames){
  if(offset===music)e.lxr_stop();const n=Math.min(4096,(offset<music?music:frames)-offset);e.lxr_process(n);
  if(audio){if(view?.buffer!==e.memory.buffer)view=new Float32Array(e.memory.buffer,e.lxr_buffer(),8192);pcm.set(view.subarray(0,n*2),offset*2);}
  for(let i=0;i<e.lxr_event_count();i++)hits.push({frame:e.lxr_event(i,0),track:e.lxr_event(i,1),note:e.lxr_event(i,2),velocity:e.lxr_event(i,3)});e.lxr_events_clear();offset+=n;
  if(++blocks%12===0){onProgress(offset/frames);await sleep();}
 }
 return {pcm,hits,sr,seconds:frames/sr,beats};
}
export function wavBytes(pcm,sr){
 const b=new Uint8Array(44+pcm.length*2),v=new DataView(b.buffer),str=(p,s)=>{for(let i=0;i<s.length;i++)v.setUint8(p+i,s.charCodeAt(i));};
 str(0,'RIFF');v.setUint32(4,b.length-8,true);str(8,'WAVE');str(12,'fmt ');v.setUint32(16,16,true);v.setUint16(20,1,true);v.setUint16(22,2,true);v.setUint32(24,sr,true);v.setUint32(28,sr*4,true);v.setUint16(32,4,true);v.setUint16(34,16,true);str(36,'data');v.setUint32(40,pcm.length*2,true);
 pcm.forEach((x,i)=>{if(!Number.isFinite(x))throw new Error('Audio render failed');v.setInt16(44+i*2,Math.round(Math.max(-1,Math.min(1,x))*32767),true);});return b;
}
const varlen=n=>{n=Math.max(0,Math.round(n));const a=[n&127];while(n>>=7)a.unshift((n&127)|128);return a;};
const text=s=>[...new TextEncoder().encode(s)];
function chunk(name,data){const out=new Uint8Array(8+data.length),v=new DataView(out.buffer);out.set(text(name),0);v.setUint32(4,data.length);out.set(data,8);return out;}
export function midiBytes(hits,bpm,beats,{gateNote=null,names=[]}={}){
 const ticks=960,end=Math.round(beats*ticks),tempo=Math.round(60000000/bpm),tracks=[];
 const eventTrack=(events,name)=>{
  const n=text(name),data=[0,255,3,...varlen(n.length),...n];let last=0;
  events.sort((a,b)=>a.tick-b.tick||a.order-b.order);for(const e of events){data.push(...varlen(e.tick-last),...e.data);last=e.tick;}
  data.push(...varlen(Math.max(last,end)-last),255,47,0);tracks.push(chunk('MTrk',data));
 };
 eventTrack([{tick:0,order:0,data:[255,81,3,(tempo>>16)&255,(tempo>>8)&255,tempo&255]},{tick:0,order:1,data:[255,88,4,4,2,24,8]}],'DXM-088');
 if(gateNote!==null){eventTrack([{tick:0,order:1,data:[144,gateNote,127]},{tick:end,order:0,data:[128,gateNote,0]}],'DXM-088 pattern gate');}
 else for(let t=0;t<7;t++){
  const events=[],notes=[36,41,43,38,39,42,46];
  const row=hits.filter(h=>h.track===t&&h.frame/44100*bpm/60<beats).map(h=>({...h,tick:Math.max(0,Math.round(h.frame/44100*bpm/60*ticks)),key:Math.max(0,Math.min(127,notes[t]+h.note))}));
  row.forEach((h,i)=>{const next=row.slice(i+1).find(x=>x.key===h.key)?.tick??end;events.push({tick:h.tick,order:1,data:[153,h.key,Math.max(1,Math.round(h.velocity*127))]},{tick:Math.max(h.tick+1,Math.min(end,next,h.tick+60)),order:0,data:[137,h.key,0]});});eventTrack(events,names[t]||`Track ${t+1}`);
 }
 const header=chunk('MThd',[0,1,(tracks.length>>8)&255,tracks.length&255,(ticks>>8)&255,ticks&255]),out=new Uint8Array(header.length+tracks.reduce((n,t)=>n+t.length,0));out.set(header);let at=header.length;for(const t of tracks){out.set(t,at);at+=t.length;}return out;
}
export function zipBytes(files){
 const crcTable=Uint32Array.from({length:256},(_,n)=>{for(let k=0;k<8;k++)n=n&1?0xedb88320^(n>>>1):n>>>1;return n>>>0;});
 const crc=bytes=>{let c=0xffffffff;for(const b of bytes)c=crcTable[(c^b)&255]^(c>>>8);return (c^0xffffffff)>>>0;};
 const chunks=[],central=[];let offset=0;
 for(const f of files){const name=new TextEncoder().encode(f.name),c=crc(f.bytes),local=new Uint8Array(30+name.length),v=new DataView(local.buffer);v.setUint32(0,0x04034b50,true);v.setUint16(4,20,true);v.setUint16(6,0x800,true);v.setUint32(14,c,true);v.setUint32(18,f.bytes.length,true);v.setUint32(22,f.bytes.length,true);v.setUint16(26,name.length,true);local.set(name,30);chunks.push(local,f.bytes);
  const directory=new Uint8Array(46+name.length),d=new DataView(directory.buffer);d.setUint32(0,0x02014b50,true);d.setUint16(4,20,true);d.setUint16(6,20,true);d.setUint16(8,0x800,true);d.setUint32(16,c,true);d.setUint32(20,f.bytes.length,true);d.setUint32(24,f.bytes.length,true);d.setUint16(28,name.length,true);d.setUint32(42,offset,true);directory.set(name,46);central.push(directory);offset+=local.length+f.bytes.length;
 }
 const centralSize=central.reduce((n,b)=>n+b.length,0),end=new Uint8Array(22),v=new DataView(end.buffer);v.setUint32(0,0x06054b50,true);v.setUint16(8,files.length,true);v.setUint16(10,files.length,true);v.setUint32(12,centralSize,true);v.setUint32(16,offset,true);const out=new Uint8Array(offset+centralSize+22);let at=0;for(const b of [...chunks,...central,end]){out.set(b,at);at+=b.length;}return out;
}
export function readWav(buffer){
 const v=new DataView(buffer),str=(i,n)=>String.fromCharCode(...new Uint8Array(buffer,i,n));if(v.byteLength<44||str(0,4)!=='RIFF'||str(8,4)!=='WAVE')throw new Error('Choose a PCM or float WAV file.');
 let format,channels,rate,bits,align,dataOffset,size;
 for(let at=12;at+8<=v.byteLength;){const id=str(at,4),n=v.getUint32(at+4,true);if(at+8+n>v.byteLength)throw new Error('WAV file is truncated.');if(id==='fmt '&&n>=16){format=v.getUint16(at+8,true);channels=v.getUint16(at+10,true);rate=v.getUint32(at+12,true);align=v.getUint16(at+20,true);bits=v.getUint16(at+22,true);if(format===65534&&n>=40)format=v.getUint16(at+32,true);}if(id==='data'){dataOffset=at+8;size=n;}at+=8+n+(n%2);}
 if(!dataOffset||![1,3].includes(format)||![1,2].includes(channels)||rate<8000||rate>192000||![8,16,24,32].includes(bits)||align!==channels*bits/8||(format===3&&bits!==32))throw new Error('Supported: mono/stereo WAV, PCM 8/16/24/32 or float 32.');
 const frames=Math.floor(size/align);if(!frames||frames>960000||frames/rate>10)throw new Error('Use a sample up to 10 seconds (960,000 frames).');const pcm=new Float32Array(frames);
 for(let i=0;i<frames;i++){let value=0;for(let c=0;c<channels;c++){const at=dataOffset+i*align+c*bits/8;let x;if(format===3)x=v.getFloat32(at,true);else if(bits===8)x=(v.getUint8(at)-128)/128;else if(bits===16)x=v.getInt16(at,true)/32768;else if(bits===24){const n=v.getUint8(at)|(v.getUint8(at+1)<<8)|(v.getInt8(at+2)<<16);x=n/8388608;}else x=v.getInt32(at,true)/2147483648;if(!Number.isFinite(x))throw new Error('WAV contains invalid samples.');value+=x/channels;}pcm[i]=Math.max(-1,Math.min(1,value));}
 return {pcm,rate};
}
