export const kits=['CARBON','ALLOY','CIRCUIT','DUST','INIT'];
export const tracks=['Drum 1','Drum 2','Drum 3','Snare','Clap / Cym','Closed HH','Open HH'];
export const shortTracks=['DRUM 1','DRUM 2','DRUM 3','SNARE','CLP / CYM','CL HH','OP HH'];
export const keys=['A','S','D','F','G','H','J'];
const wave=['Sine','Triangle','Saw','Square','Noise','PWM'];
export const defs=[
 ['Pitch','hz','log'],['Waveform','enum',wave],['Decay','time','log'],['Attack','time','log'],['Slope','pct'],['Pitch sweep','st'],['Pitch decay','time','log'],['FM ratio','ratio','log'],['FM amount','pct'],['FM waveform','enum',wave],['Noise mix','pct'],['Click level','pct'],['Cutoff','hz','log'],['Resonance','pct'],['Filter type','enum',['Bypass','Lowpass','Highpass','Bandpass','Unit BP','Notch','Peak','LP2']],['Drive','pct'],['Distortion','enum',['Off','Shape','Fold','Clip']],['Reduction','pct'],['Pan','pan'],['Voice level','db'],['LFO rate','hz','log'],['LFO depth','pct'],['LFO target','enum',['Pitch','Cutoff','FM amount','Amplitude']],['LFO wave','enum',['Sine','Triangle','Saw','Square','Random']],['FM ratio 2','ratio','log'],['FM gain 2','pct'],['Pulse width','pct'],['Transient','enum',['Snappy','Offset','Click','Click 2','Tick','Kick','Rim','Drip','Hat','Clap','Kick 2','Snare','Tom','Snap']],['Repeat','integer'],['Open decay','time','log'],['Velocity target','enum',['Off','Pitch','Cutoff','FM amount','Decay']],['Velocity amount','signedpct'],['OSC mode','enum',['FM','Mix']],['LFO retrigger','enum',['Free','Retrigger']],['Transient rate','ratio','log'],['Noise color','hz','log'],['Filter drive','pct'],['Pitch slope','pct'],['FM wave 2','enum',wave],
].map(([name,unit,scale],id)=>({id,name,unit,options:Array.isArray(scale)?scale:null,log:scale==='log',min:0,max:1}));
export const pages=[
 {name:'VOICE',params:[0,2,5,8,7,10,11,12]},
 {name:'OSC',params:[0,1,26,10,35,32,11,27]},
 {name:'ENV',params:[3,2,4,5,6,28,29,31]},
 {name:'FM',params:[7,8,9,24,25,32,5,6]},
 {name:'FILTER',params:[12,13,14,36,15,16,17,19]},
 {name:'MOD',params:[20,21,22,23,33,30,31,37]},
 {name:'CLICK',params:[11,27,34,3,2,5,18,19]},
];
export const clamp=(x,a,b)=>Math.min(b,Math.max(a,x));
export function bindDefinitions(e){for(const d of defs){d.min=e.lxr_param_min(d.id);d.max=e.lxr_param_max(d.id);}}
export function constrain(d,v){v=clamp(v,d.min,d.max);return d.options||d.unit==='integer'?Math.round(v):v;}
export function normalize(d,x){return d.log?Math.log((x+0.001)/(d.min+0.001))/Math.log((d.max+0.001)/(d.min+0.001)):(x-d.min)/(d.max-d.min);}
export function denormalize(d,x){return constrain(d,d.log?(d.min+0.001)*Math.pow((d.max+0.001)/(d.min+0.001),clamp(x,0,1))-0.001:d.min+clamp(x,0,1)*(d.max-d.min));}
export function format(d,v){
 if(d.options)return d.options[Math.round(v)]||d.options[0];
 switch(d.unit){
  case 'hz':return v>=1000?`${(v/1000).toFixed(v>=10000?1:2)} kHz`:`${v<10?v.toFixed(2):Math.round(v)} Hz`;
  case 'time':return v>=1?`${v.toFixed(2)} s`:`${v<0.01?(v*1000).toFixed(1):Math.round(v*1000)} ms`;
  case 'pct':return `${Math.round(v*100)}%`;
  case 'signedpct':return `${v>0?'+':''}${Math.round(v*100)}%`;
  case 'ratio':return `${v.toFixed(2)}×`;
  case 'st':return `${v>0?'+':''}${v.toFixed(1)} st`;
  case 'pan':return Math.abs(v)<0.02?'CENTER':`${Math.round(Math.abs(v)*100)} ${v<0?'L':'R'}`;
  case 'db':return v<0.001?'−∞ dB':`${(20*Math.log10(v)).toFixed(1)} dB`;
  default:return `${Math.round(v)}`;
 }
}
