// Host formats and WebView bridge. All sound and sequencer timing are in Rust.
#include "LXR02.h"
#include "IPlug_include_in_plug_src.h"
#include "KitStore.h"
#include <algorithm>
#include <cmath>
#include <limits>
#include <cstring>
#include <unordered_map>
#ifdef OS_WIN
#include <windows.h>
#include <filesystem>
static void DXMModuleAnchor() {}
#endif
using namespace iplug;
#ifdef OS_MAC
extern "C" bool lxr_prepare_export(const char*,const char*,char*,size_t);
extern "C" bool lxr_drag_exports(void*,const char*);
#endif
LXR02::LXR02(const InstanceInfo& info):iplug::Plugin(info,MakeConfig(kParams,1)) {
#ifdef CLAP_API
  mClapHost=info.mHost;
#endif
  mUISent.fill(std::numeric_limits<float>::quiet_NaN());
  mModel=lxrp_create(44100.f);mStatus[1]=-1;for(int t=0;t<7;t++)mStatus[6+t]=-1;
  for(int p=0;p<kParams;p++){
    char name[64];float bounds[3];lxrp_info(p,name,sizeof(name),bounds);
    const double value=lxrp_get(mModel,p);
    if(bounds[2])GetParam(p)->InitInt(name,(int)value,(int)bounds[0],(int)bounds[1]);
    else GetParam(p)->InitDouble(name,value,bounds[0],bounds[1],.000001);
  }
  GetParam(283)->SetDisplayText(0,"Patterns");GetParam(283)->SetDisplayText(1,"Drums");
  GetParam(284)->SetDisplayText(0,"Mix");GetParam(284)->SetDisplayText(1,"Separate");
  mParamCache.fill(std::numeric_limits<float>::quiet_NaN());
#ifdef WEBVIEW_EDITOR_DELEGATE
  SetMaxJSStringLength(128000000);
  mEditorInitFunc=[this](){
#ifdef OS_WIN
    // Resolve from this module, never from the DAW's working directory.
    HMODULE module=nullptr;
    wchar_t modulePath[32768]{};
    if(GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        reinterpret_cast<LPCWSTR>(&DXMModuleAnchor),&module)&&GetModuleFileNameW(module,modulePath,32768)) {
      auto folder=std::filesystem::path(modulePath).parent_path();
#ifdef VST3_API
      auto index=folder.parent_path()/L"Resources"/L"web"/L"index.html";
#else
      auto index=folder/L"DXM-088.resources"/L"web"/L"index.html";
#endif
      LoadFile(index.u8string().c_str(),nullptr);
    }
#else
    LoadIndexHtml(__FILE__,GetBundleID());
#endif
    EnableScroll(true);
  };
#endif
  MakeDefaultPreset("CARBON",1);SetTailSize(192000*30);
}
void LXR02::Retire(Ready* r){if(r){lxrp_destroy(r->engine);delete r;}}
LXR02::~LXR02(){Retire(mActive);Retire(mPending.exchange(nullptr));Retire(mRetired.exchange(nullptr));lxrp_destroy(mModel);}
void LXR02::SyncModel(int pattern) const {
  // MIDI is rendered in the audio instance; editor/state edits belong to the
  // displayed slot, not whichever slot the model last selected manually.
  if(pattern==-1&&!mPending.load()&&GetParam(283)->Value()<.5&&mStatus[1].load()>=0)pattern=(int)mStatus[0].load();
  if(pattern>=0&&pattern<128){std::array<double,8> args{};args[0]=pattern;lxrp_command(mModel,2,args.data());}
  for(int p=0;p<kParams;p++){const float v=(float)GetParam(p)->Value();if(std::abs(v-lxrp_get(mModel,p))>1e-7)lxrp_set(mModel,p,v);}
}
void LXR02::PublishModel(){
  auto* r=new Ready{lxrp_clone(mModel,mRate.load()),++mRevision};
  Retire(mPending.exchange(r,std::memory_order_acq_rel));
}
void LXR02::OnReset(){
  std::lock_guard<std::mutex> guard(mModelMutex);mRate=(float)GetSampleRate();SyncModel(-2);
  Retire(mActive);mActive=new Ready{lxrp_clone(mModel,mRate),++mRevision};
  Retire(mPending.exchange(nullptr));Retire(mRetired.exchange(nullptr));
  for(int p=0;p<kParams;p++)mParamCache[p]=(float)GetParam(p)->Value();mMidiCount=0;
  const int latency=(int)std::lround(15.5*GetSampleRate()/44140.625);if(latency!=GetLatency())SetLatency(latency);
}
void LXR02::ProcessMidiMsg(const IMidiMsg& msg){
  if(mMidiCount<(int)mMidi.size()){
    // Keep offsets ordered even if a host mixes native notes and raw MIDI.
    int i=mMidiCount++;while(i>0&&mMidi[i-1].mOffset>msg.mOffset){mMidi[i]=mMidi[i-1];--i;}mMidi[i]=msg;
  }else mOverflow=true;
}
void LXR02::GetBusName(ERoute direction,int bus,int nBuses,WDL_String& name) const {
  static const char* names[]={"Mix","Drum 1","Drum 2","Drum 3","Snare","Cymbal","Closed Hat","Open Hat"};
  if(direction==ERoute::kOutput&&bus>=0&&bus<8)name.Set(names[bus]);
  else iplug::Plugin::GetBusName(direction,bus,nBuses,name);
}
void LXR02::ProcessBlock(sample**,sample** out,int nFrames){
  if(!mRetired.load(std::memory_order_acquire)){
    if(auto* ready=mPending.exchange(nullptr,std::memory_order_acq_rel)){
      if(mActive)lxrp_inherit(ready->engine,mActive->engine);
      mRetired.store(mActive,std::memory_order_release);mActive=ready;
      for(int p=0;p<kParams;p++)mParamCache[p]=lxrp_get(mActive->engine,p);
    }
  }
  if(!mActive){for(int c=0;c<kChannels;c++)if(IsChannelConnected(ERoute::kOutput,c))std::fill(out[c],out[c]+nFrames,0.);return;}
  auto* engine=mActive->engine;
  uint32_t read=mRead.load(std::memory_order_relaxed);const auto write=mWrite.load(std::memory_order_acquire);
  while(read!=write){const auto& cmd=mCommands[read%kQueue];if(cmd.revision>mActive->revision){lxrp_command(engine,cmd.op,cmd.args.data());if(cmd.parameter>=0&&std::abs(GetParam(cmd.parameter)->Value()-cmd.value)<1e-6)mParamCache[cmd.parameter]=cmd.value;}++read;}
  mRead.store(read,std::memory_order_release);
  for(int p=0;p<kParams;p++){
    const float v=(float)GetParam(p)->Value();if(v!=mParamCache[p]){lxrp_set(engine,p,v);mParamCache[p]=v;}
  }
  const double bpm=std::isfinite(GetTempo())&&GetTempo()>0.?GetTempo():128.;
  const double beat=std::isfinite(GetPPQPos())?GetPPQPos():0.;const bool running=GetTransportIsRunning();
  lxrp_transport(engine,bpm,beat,running);
  if(mOverflow.exchange(false)){std::array<double,8> a{};lxrp_command(engine,14,a.data());mMidiCount=0;}
  int offset=0,event=0,lastFrames=0;
  while(offset<nFrames){
    while(event<mMidiCount&&mMidi[event].mOffset<=offset){const auto& m=mMidi[event++];lxrp_midi(engine,m.mStatus,m.mData1,m.mData2);}
    int end=std::min(nFrames,offset+4096);
    if(event<mMidiCount)end=std::min(end,mMidi[event].mOffset);
    if(end<=offset)continue;
    const int n=end-offset;lastFrames=n;lxrp_process_multi(engine,mAudio.data(),n);
    // Buses may be enabled sparsely, e.g. Mix + only Snare. Channel indices
    // remain fixed; NOutChansConnected() assumes consecutive connections.
    for(int c=0;c<kChannels;c++)if(IsChannelConnected(ERoute::kOutput,c))
      for(int i=0;i<n;i++)out[c][offset+i]=mAudio[i*kChannels+c];
    offset=end;
  }
  int remain=0;for(int i=event;i<mMidiCount;i++){mMidi[i].mOffset-=nFrames;mMidi[remain++]=mMidi[i];}mMidiCount=remain;
  float s[20];lxrp_status(engine,s);for(int i=0;i<20;i++)mStatus[i].store(s[i],std::memory_order_relaxed);
  mPreview.store(lxrp_preview(engine),std::memory_order_relaxed);
  if(lastFrames>0)for(int i=0;i<128;i++){
    int at=i*(lastFrames-1)/127;float wave=0.;
    const int first=mParamCache[284]>=.5?2:0,last=first?16:2;
    for(int c=first;c<last;c++)wave+=mAudio[at*kChannels+c]*.5f;
    mScope[i].store(wave,std::memory_order_relaxed);
  }
}
std::string LXR02::ProjectJSON() const {
  SyncModel();std::string s(lxrp_json(mModel,nullptr,0),'\0');lxrp_json(mModel,(uint8_t*)s.data(),s.size());return s;
}
bool LXR02::SerializeState(IByteChunk& chunk) const {
  std::lock_guard<std::mutex> guard(mModelMutex);const auto s=ProjectJSON();
  const uint32_t magic=0x4c585236;const int size=(int)s.size();chunk.Put(&magic);chunk.Put(&size);chunk.PutBytes(s.data(),size);return SerializeParams(chunk);
}
int LXR02::UnserializeState(const IByteChunk& chunk,int pos){
  uint32_t magic=0;int size=0;pos=chunk.Get(&magic,pos);if(pos<0||(magic!=0x4c585233&&magic!=0x4c585234&&magic!=0x4c585235&&magic!=0x4c585236))return -1;
  pos=chunk.Get(&size,pos);if(pos<0||size<=0||size>64000000||size>chunk.Size()-pos)return -1;
  std::string s(size,'\0');pos=chunk.GetBytes(s.data(),size,pos);if(pos<0)return -1;
  std::lock_guard<std::mutex> guard(mModelMutex);
  if(!lxrp_load(mModel,(const uint8_t*)s.data(),s.size()))return -1;
  const int count=magic==0x4c585233?274:magic==0x4c585234?283:magic==0x4c585235?292:kParams;
  for(int p=0;p<count;p++){double v;pos=chunk.Get(&v,pos);if(pos<0||!std::isfinite(v))return -1;GetParam(p)->Set(v);}
  for(int p=count;p<kParams;p++)GetParam(p)->Set(lxrp_get(mModel,p));
  SyncModel(-2);PublishModel();mProjectDirty=true;return pos;
}
void LXR02::NotifyParams(){
  for(int p=0;p<kParams;p++){
    const double v=lxrp_get(mModel,p);
    if(std::abs(v-GetParam(p)->Value())>1e-7){
      if(!mUIGestures[p])BeginInformHostOfParamChangeFromUI(p);
      SendParameterValueFromUI(p,GetParam(p)->ToNormalized(v));
      if(!mUIGestures[p])EndInformHostOfParamChangeFromUI(p);
    }
  }
}
void LXR02::SendProject(){
#ifdef WEBVIEW_EDITOR_DELEGATE
  std::lock_guard<std::mutex> guard(mModelMutex);auto s=ProjectJSON();auto p=nlohmann::json::parse(s,nullptr,false);
  if(!p.is_discarded())SendJSONFromDelegate({{"type","project"},{"project",p}});SendCurrentParamValuesFromDelegate();
#endif
}
void LXR02::OnUIOpen(){}
void LXR02::EndUIGestures(){for(int p=0;p<kParams;p++)if(mUIGestures[p]){mUIGestures[p]=false;EndInformHostOfParamChangeFromUI(p);}}
void LXR02::CloseWindow(){EndUIGestures();mEditorReady=false;iplug::Plugin::CloseWindow();}
void LXR02::OnIdle(){
  Retire(mRetired.exchange(nullptr,std::memory_order_acq_rel));
#ifdef WEBVIEW_EDITOR_DELEGATE
  if(!mEditorReady)return;
  if(mProjectDirty.exchange(false))SendProject();
  nlohmann::json values=nlohmann::json::array();
  for(int p=0;p<kParams;p++){float v=(float)GetParam(p)->Value();if(v!=mUISent[p]){values.push_back({{"id",p},{"value",v}});mUISent[p]=v;}}
  if(!values.empty())SendJSONFromDelegate({{"type","parameters"},{"values",values}});
  nlohmann::json s; s["type"]="transport";s["pattern"]=mStatus[0].load();s["note"]=mStatus[1].load();s["left"]=mStatus[2].load();s["right"]=mStatus[3].load();s["bpm"]=mStatus[4].load();s["running"]=mStatus[5].load();
  s["preview"]=mPreview.load();s["positions"]=nlohmann::json::array();for(int t=0;t<7;t++)s["positions"].push_back(mStatus[6+t].load());
  s["meters"]=nlohmann::json::array();for(int t=0;t<7;t++)s["meters"].push_back(mStatus[13+t].load());
  s["wave"]=nlohmann::json::array();for(int t=0;t<128;t++)s["wave"].push_back(mScope[t].load());SendJSONFromDelegate(s);
#endif
}
bool LXR02::OnMessage(int tag,int,int size,const void* data){
  if(tag==101){mUISent.fill(std::numeric_limits<float>::quiet_NaN());mEditorReady=true;mProjectDirty=true;return true;}
  if(tag!=100||size<=0||size>64000000)return false;
  auto j=nlohmann::json::parse((const char*)data,(const char*)data+size,nullptr,false);if(j.is_discarded())return false;
#ifdef OS_MAC
  if(j.value("type","")=="prepareFiles"){
    if(!j["files"].is_array()||j["files"].empty()||j["files"].size()>7)return false;
    std::vector<std::string> paths;
    for(const auto& f:j["files"]){if(!f["name"].is_string()||!f["data"].is_string())return false;char path[32768]{};const auto name=f["name"].get<std::string>(),data=f["data"].get<std::string>();if(!lxr_prepare_export(name.c_str(),data.c_str(),path,sizeof(path)))return false;paths.emplace_back(path);}
    if(j.value("first",true))mExportPaths.clear();mExportPaths.insert(mExportPaths.end(),paths.begin(),paths.end());
    if(j.value("last",true)){mExportToken=std::to_string(++mRevision);SendJSONFromDelegate({{"type","files"},{"token",mExportToken}});}return true;
  }
  if(j.value("type","")=="dragFiles"){
    if(j.value("token","")!=mExportToken||mExportPaths.empty())return false;
    const auto paths=nlohmann::json(mExportPaths).dump();const bool started=lxr_drag_exports(iplug::WebViewEditorDelegate::mView,paths.c_str());if(!started)SendJSONFromDelegate({{"type","exportError"},{"message","Hold and drag the button, or use Save file."}});return started;
  }
#endif
  if(j.value("type","")=="kitLibrary"){
    const auto request=j.value("request",0u);
    try{
      dxm::KitStore store;const auto operation=j.value("operation","");nlohmann::json result;
      if(operation=="list")result=store.list();
      else if(operation=="get")result=store.get(j.at("id").get<std::string>());
      else if(operation=="save"){store.save(j.at("kit"));result={{"id",j["kit"].at("id")}};}
      else throw std::runtime_error("Unknown kit operation.");
      SendJSONFromDelegate({{"type","kitLibrary"},{"request",request},{"ok",true},{"result",result}});
    }catch(const std::exception&){SendJSONFromDelegate({{"type","kitLibrary"},{"request",request},{"ok",false},{"message","Kit could not be saved or loaded. Check the user kit folder."}});}
    return true;
  }
  std::lock_guard<std::mutex> guard(mModelMutex);SyncModel(j.value("pattern",-1));
  if(j.value("type","")=="gesture"){
    if(!j["id"].is_number_unsigned()||!j["active"].is_boolean())return false;
    const auto id=j["id"].get<unsigned>();if(id>=kParams)return false;
    const bool active=j["active"].get<bool>();if(active!=mUIGestures[id]){
      mUIGestures[id]=active;if(active)BeginInformHostOfParamChangeFromUI(id);else EndInformHostOfParamChangeFromUI(id);
    }return true;
  }
  if(j.value("type","")=="meta"){const auto name=j.value("name",std::string("Session 01"));lxrp_meta(mModel,(const uint8_t*)name.data(),name.size(),j.value("kit",0));if(j.contains("labels")){const auto labels=j["labels"].dump();lxrp_labels(mModel,(const uint8_t*)labels.data(),labels.size());}DirtyParametersFromUI();return true;}
  if(j.value("type","")=="project"){
    const auto s=j["project"].dump();if(!lxrp_load(mModel,(const uint8_t*)s.data(),s.size()))return false;
    NotifyParams();PublishModel();DirtyParametersFromUI();return true;
  }
  if(j.value("type","")=="parameter"){
    if(!j["id"].is_number_unsigned()||!j["value"].is_number())return false;const int id=j["id"].get<int>();if(id>=kParams)return false;
    lxrp_set(mModel,id,j["value"].get<float>());NotifyParams();return true;
  }
  if(j.value("type","")!="commands"||!j["commands"].is_array()||j["commands"].size()>15000)return false;
  static const std::unordered_map<std::string,uint32_t> ops={{"lxr_param",0},{"lxr_tempo",1},{"lxr_pattern",2},{"lxr_length",3},{"lxr_set_step",4},{"lxr_set_lock",5},{"lxr_level",6},{"lxr_mute",7},{"lxr_solo",8},{"lxr_fx",9},{"lxr_morph",10},{"lxr_load_kit",11},{"lxr_trigger",12},{"lxr_panic",13},{"lxr_stop",14},{"lxr_play",15},{"lxr_generate",16},{"lxr_track_settings",17},{"lxr_step_extras",18},{"lxr_route",19},{"lxr_macro",20},{"lxr_performance",21},{"lxr_scene_capture",22},{"lxr_scene_enable",23},{"lxr_sample_settings",24},{"lxr_track_swing",29},{"lxr_route_source",30}};
  bool full=j["commands"].size()>64;
  const int pattern=j.value("pattern",-1);
  auto enqueue=[&](Command command){command.revision=++mRevision;const auto w=mWrite.load(std::memory_order_relaxed);if(w-mRead.load(std::memory_order_acquire)<kQueue){mCommands[w%kQueue]=command;mWrite.store(w+1,std::memory_order_release);}else full=true;};
  for(const auto& c:j["commands"]){
    if(!c.is_array()||c.size()!=2||!c[0].is_string()||!c[1].is_array()||c[1].size()>8)return false;
    auto op=ops.find(c[0].get<std::string>());if(op==ops.end())return false;
    Command command{};command.op=op->second;for(size_t i=0;i<c[1].size();i++){if(!c[1][i].is_number())return false;command.args[i]=c[1][i].get<double>();if(!std::isfinite(command.args[i]))return false;}
    lxrp_command(mModel,command.op,command.args.data());
    // A scene edit must reach that exact slot even when its physical value is
    // equal to the last host value, or MIDI changes slot in the same block.
    if(!full&&pattern>=0&&pattern<128&&lxrp_scene_enabled(mModel,pattern)){
      auto scene=[&](uint32_t op,std::array<double,8> args,int parameter){if(parameter<0||parameter>=kParams)return;Command c{};c.op=op;c.args=args;c.parameter=parameter;c.value=lxrp_get(mModel,parameter);enqueue(c);};
      const auto& a=command.args;
      if(command.op==0){scene(25,{double(pattern),a[0],a[1],a[2]},int(a[1])<39?int(a[0])*39+int(a[1]):292+int(a[0])*17+int(a[1])-39);continue;}
      if(command.op>=6&&command.op<=8){scene(26,{double(pattern),a[0],double(command.op-6),a[1]},(command.op==6?249:command.op==7?256:263)+int(a[0]));continue;}
      if(command.op==9){scene(27,{double(pattern),a[0],a[1]},234+int(a[0]));continue;}
      if(command.op==10){scene(28,{double(pattern),4.,a[0]},271);scene(28,{double(pattern),5.,a[1]},272);continue;}
      if(command.op==20){scene(28,{double(pattern),a[0],a[1]},274+int(a[0]));continue;}
    }
    // Parameters travel through the host; the queue carries sequencer edits and pad triggers.
    if(!full&&(command.op==2||command.op==3||command.op==4||command.op==5||command.op==12||command.op==13||command.op==14||command.op==15||command.op==16||command.op==17||command.op==18||command.op==19||command.op==21||command.op==22||command.op==23||command.op==24)){
      enqueue(command);
    }
  }
  NotifyParams();if(full)PublishModel();DirtyParametersFromUI();return true;
}

#ifdef CLAP_API
// CLAP streams may consume fewer bytes than requested. Keep state I/O bounded.
bool LXR02::stateSave(const clap_ostream* stream) noexcept {
  IByteChunk chunk;if(!SerializeState(chunk))return false;
  size_t at=0;while(at<(size_t)chunk.Size()){
    const auto n=stream->write(stream,chunk.GetData()+at,chunk.Size()-at);
    if(n<=0||n>chunk.Size()-(int64_t)at)return false;at+=(size_t)n;
  }return true;
}
bool LXR02::stateLoad(const clap_istream* stream) noexcept {
  IByteChunk chunk;uint8_t buffer[4096];int64_t n;
  while((n=stream->read(stream,buffer,sizeof(buffer)))>0){if(n>(int64_t)sizeof(buffer)||chunk.Size()+n>33000000)return false;chunk.PutBytes(buffer,(int)n);}
  if(n<0||UnserializeState(chunk,0)<0)return false;
  OnRestoreState();
  if(mClapHost){auto* params=(const clap_host_params_t*)mClapHost->get_extension(mClapHost,CLAP_EXT_PARAMS);if(params)params->rescan(mClapHost,CLAP_PARAM_RESCAN_VALUES);}
  return true;
}
bool LXR02::audioPortsGetConfig(uint32_t index,clap_audio_ports_config* config) const noexcept {
  if(index>7)return false;*config={};config->id=index;std::strncpy(config->name,index==0?"Mix + 7 stereo drums":index==1?"Stereo":"Stereo + drum outputs",CLAP_NAME_SIZE);
  config->input_port_count=0;config->output_port_count=index==0?8:index;config->has_main_input=false;config->has_main_output=true;
  config->main_output_channel_count=2;config->main_output_port_type=CLAP_PORT_STEREO;return true;
}
bool LXR02::notePortsInfo(uint32_t index,bool input,clap_note_port_info* info) const noexcept {
  if(index!=0||!input)return false;*info={};info->id=0;info->supported_dialects=CLAP_NOTE_DIALECT_MIDI;info->preferred_dialect=CLAP_NOTE_DIALECT_MIDI;
  std::strncpy(info->name,"Pattern / drum notes",CLAP_NAME_SIZE);return true;
}
void LXR02::reset() noexcept {
  if(mActive){std::array<double,8> args{};lxrp_command(mActive->engine,13,args.data());}mMidiCount=0;
}
#endif

#ifdef WEBVIEW_EDITOR_DELEGATE
extern "C" bool lxr_save_project_dialog(const char*,char*,size_t);
bool LXR02::OnCanDownloadMIMEType(const char* type){return std::strcmp(type,"application/json")==0||std::strcmp(type,"audio/wav")==0||std::strcmp(type,"audio/midi")==0||std::strcmp(type,"application/zip")==0||std::strcmp(type,"audio/x-wav")==0;}
void LXR02::OnGetLocalDownloadPathForFile(const char* name,WDL_String& path){char chosen[32768]{};if(lxr_save_project_dialog(name,chosen,sizeof(chosen)))path.Set(chosen);else path.Set("");}
void LXR02::OnDownloadedFile(const char*){SendJSONFromDelegate({{"type","saved"}});}
#endif
