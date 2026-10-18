// WKWebView / WebView2 bridge. Browser sessions retain their own audio engine.
(() => {
  const native=typeof globalThis.IPlugSendMsg==='function';
  const preview=new URLSearchParams(location.search).has('pluginPreview');
  if(!native&&!preview)return;
  const emit=(type,data)=>dispatchEvent(new CustomEvent(`lxr-native-${type}`,{detail:data}));
  const send=data=>{
    if(!native)return;
    const bytes=new TextEncoder().encode(JSON.stringify(data));let binary='';
    for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));
    IPlugSendMsg({msg:'SAMFUI',msgTag:100,ctrlTag:-1,data:btoa(binary)});
  };
  let resolveReady,kitSerial=0;const initial=new Promise(r=>resolveReady=r),kitRequests=new Map();
  // Physical values arrive as JSON, preserving precision beyond the SDK's %f bridge.
  globalThis.SPVFD=()=>{};
  globalThis.SAMFD=(_tag,_size,encoded)=>{
    try{const data=JSON.parse(new TextDecoder().decode(Uint8Array.from(atob(encoded),c=>c.charCodeAt(0))));
      if(data.type==='project'){resolveReady(data.project);emit('project',data.project);}
      else if(data.type==='transport')emit('transport',data);
      else if(data.type==='saved')emit('saved',data);
      else if(data.type==='files')emit('files',data);
      else if(data.type==='exportError')emit('export-error',data);
      else if(data.type==='parameters')for(const p of data.values)emit('parameter',{...p,physical:true});
      else if(data.type==='kitLibrary'){const pending=kitRequests.get(data.request);if(pending){clearTimeout(pending.timer);kitRequests.delete(data.request);data.ok?pending.resolve(data.result):pending.reject(new Error(data.message));}}
    }catch(e){console.error(e);}
  };
  globalThis.SCVFD=globalThis.SCMFD=globalThis.SMMFD=globalThis.SSMFD=()=>{};
  globalThis.lxrNative={preview,ready(){if(native)IPlugSendMsg({msg:'SAMFUI',msgTag:101,ctrlTag:-1,data:''});else resolveReady(null);return initial;},
    commands:(commands,pattern)=>send({type:'commands',commands,pattern}),project:project=>send({type:'project',project}),
    parameter:(id,value,pattern)=>send({type:'parameter',id,value,pattern}),gesture:(id,active)=>send({type:'gesture',id,active}),prepareFiles:files=>{for(let i=0;i<files.length;i++)send({type:'prepareFiles',files:[files[i]],first:i===0,last:i===files.length-1});},dragFiles:token=>send({type:'dragFiles',token}),meta:(name,kit,labels,pattern)=>send({type:'meta',name,kit,labels,pattern}),
    kits:(operation,details={})=>new Promise((resolve,reject)=>{const request=++kitSerial,timer=setTimeout(()=>{kitRequests.delete(request);reject(new Error('Kit library did not respond.'));},15000);kitRequests.set(request,{resolve,reject,timer});send({type:'kitLibrary',operation,request,...details});})};
})();
