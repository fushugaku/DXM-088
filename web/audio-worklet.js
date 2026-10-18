import {uploadSamples} from './feature-state.js';
// Only messaging and channel copies live here. All synthesis and sequencing is Rust.
const allowed=new Set(['lxr_param','lxr_load_kit','lxr_morph','lxr_tempo','lxr_play','lxr_stop','lxr_panic','lxr_trigger','lxr_pattern','lxr_set_step','lxr_set_lock','lxr_length','lxr_generate','lxr_level','lxr_mute','lxr_solo','lxr_fx','lxr_track_settings','lxr_step_extras','lxr_route','lxr_macro','lxr_performance','lxr_scene_capture','lxr_scene_enable','lxr_scene_param','lxr_scene_mix','lxr_scene_fx','lxr_scene_misc','lxr_sample_settings']);
class LxrProcessor extends AudioWorkletProcessor {
  constructor(options){
    super();
    this.e=new WebAssembly.Instance(options.processorOptions.module,{}).exports;
    this.e.lxr_init(sampleRate);this.block=0;this.view=null;this.failed=false;
    this.port.onmessage=({data})=>{
      try {for(const [fn,args] of (data.batch||(data.command?[data.command]:[]))) if(allowed.has(fn)) this.e[fn](...args);if(data.samples)uploadSamples(this.e,data.samples);}
      catch(error){this.port.postMessage({error:String(error)});}
    };
    this.port.postMessage({ready:true,sampleRate});
  }
  process(_inputs,outputs){
    const output=outputs[0];if(!output?.[0] || this.failed)return true;
    try {
      const n=output[0].length;this.e.lxr_process(n);
      if(!this.view || this.view.buffer!==this.e.memory.buffer)this.view=new Float32Array(this.e.memory.buffer,this.e.lxr_buffer(),8192);
      for(let i=0;i<n;i++){output[0][i]=this.view[2*i];if(output[1])output[1][i]=this.view[2*i+1];}
      if(++this.block%16===0)this.port.postMessage({positions:Array.from({length:7},(_,t)=>this.e.lxr_position(t)),meters:Array.from({length:7},(_,t)=>this.e.lxr_meter(t)),peak:[this.e.lxr_peak(0),this.e.lxr_peak(1)],pattern:this.e.lxr_get_pattern(),frame:currentFrame});
    }catch(error){this.failed=true;this.port.postMessage({error:String(error)});for(const channel of output)channel.fill(0);}
    return true;
  }
}
registerProcessor('lxr02',LxrProcessor);
