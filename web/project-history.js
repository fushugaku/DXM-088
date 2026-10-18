// Keep sample data by reference and copy only the edited slots during normal edits.
export function captureHistory(state,indices=[state.pattern]){
 const slots=indices===true?state.patterns.map((_,p)=>p):[...new Set([state.pattern,...indices])];
 const {patterns,scenes,samples,...globals}=state;
 return {json:JSON.stringify({...globals,patterns:slots.map(p=>[p,patterns[p]]),scenes:slots.map(p=>[p,scenes[p]]),samples:samples.map(a=>a?{...a,data:undefined}:null)}),data:samples.map(a=>a?.data??null),slots};
}
export function restoreHistory(current,snapshot){
 const saved=JSON.parse(snapshot.json),patterns=[...current.patterns],scenes=[...current.scenes];
 for(const [p,value] of saved.patterns)patterns[p]=value;
 for(const [p,value] of saved.scenes)scenes[p]=value;
 saved.samples.forEach((a,t)=>{if(a)a.data=snapshot.data[t];});
 return {...saved,patterns,scenes};
}
