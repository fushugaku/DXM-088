// Relative motion: changing precision and reversing at an end stop never jumps.
export class KnobDrag {
 constructor(value,y){this.value=value;this.y=y;}
 move(y,fine){this.value=Math.max(0,Math.min(1,this.value+(this.y-y)/(fine?3200:320)));this.y=y;return this.value;}
}
export function wheelPixels(event){return event.deltaY*(event.deltaMode===1?30:event.deltaMode===2?300:1);}

// WKWebView can echo an earlier value while the pointer has already moved further.
// Keep a local edit until the host acknowledges its latest value, or a short timeout.
export class ParameterEdits {
 constructor(){this.pending=new Map();this.active=new Set();}
 begin(id){this.active.add(id);}
 end(id){this.active.delete(id);}
 write(id,value,now){this.pending.set(id,{value,until:now+400});}
 accept(id,value,now){
  const pending=this.pending.get(id);
  if(pending){
   if(Math.abs(pending.value-value)<=Math.max(1e-6,Math.abs(value)*1e-6)){this.pending.delete(id);return false;}
   if(this.active.has(id)||now<pending.until)return false;
   this.pending.delete(id);
  }
  return !this.active.has(id);
 }
}
