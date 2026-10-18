self.onmessage=({data})=>{
  try {
    const e=new WebAssembly.Instance(data.module,{}).exports;
    const sr=44100;e.lxr_init(sr);
    for(const [fn,args] of data.commands)e[fn](...args);
    const music=Math.round(data.length*data.loops*60/data.bpm/4*sr);
    const frames=music+sr*2;
    const buffer=new ArrayBuffer(44+frames*4);const view=new DataView(buffer);
    const text=(offset,s)=>{for(let i=0;i<s.length;i++)view.setUint8(offset+i,s.charCodeAt(i));};
    text(0,'RIFF');view.setUint32(4,36+frames*4,true);text(8,'WAVE');text(12,'fmt ');
    view.setUint32(16,16,true);view.setUint16(20,1,true);view.setUint16(22,2,true);
    view.setUint32(24,sr,true);view.setUint32(28,sr*4,true);view.setUint16(32,4,true);view.setUint16(34,16,true);
    text(36,'data');view.setUint32(40,frames*4,true);
    e.lxr_play();let offset=0,peak=0,squares=0;let samples;
    while(offset<frames){
      if(offset===music)e.lxr_stop();
      const boundary=offset<music?music:frames;const n=Math.min(4096,boundary-offset);
      e.lxr_process(n);
      if(!samples || samples.buffer!==e.memory.buffer)samples=new Float32Array(e.memory.buffer,e.lxr_buffer(),8192);
      for(let i=0;i<n*2;i++){
        const x=samples[i];if(!Number.isFinite(x))throw new Error('Non-finite audio sample');
        peak=Math.max(peak,Math.abs(x));squares+=x*x;
        view.setInt16(44+offset*4+i*2,Math.round(Math.max(-1,Math.min(1,x))*32767),true);
      }
      offset+=n;
    }
    self.postMessage({buffer,peak,rms:Math.sqrt(squares/(frames*2)),seconds:frames/sr},[buffer]);
  }catch(error){self.postMessage({error:String(error)});}
};
