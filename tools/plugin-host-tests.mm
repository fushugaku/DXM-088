// Real plug-in ABI tests. No DAW/project is opened or modified.
#include <AudioToolbox/AudioToolbox.h>
#include <CoreFoundation/CoreFoundation.h>
#include <clap/clap.h>
#include <dlfcn.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
#include "../plugin/engine.h"

#define REQUIRE(x) do{if(!(x)){std::fprintf(stderr,"FAIL line %d: %s\n",__LINE__,#x);std::exit(1);}}while(0)
struct Transport{double bpm=120.,beat=0.,sample=0.;bool playing=true;};
static OSStatus beatTempo(void* p,Float64* beat,Float64* bpm){auto&t=*static_cast<Transport*>(p);if(beat)*beat=t.beat;if(bpm)*bpm=t.bpm;return noErr;}
static OSStatus transport(void* p,Boolean* play,Boolean* changed,Float64* sample,Boolean* cycle,Float64* start,Float64* end){auto&t=*static_cast<Transport*>(p);if(play)*play=t.playing;if(changed)*changed=false;if(sample)*sample=t.sample;if(cycle)*cycle=false;if(start)*start=0.;if(end)*end=4.;return noErr;}
static OSStatus input(void*,AudioUnitRenderActionFlags*,const AudioTimeStamp*,UInt32,UInt32 frames,AudioBufferList* b){for(UInt32 c=0;c<b->mNumberBuffers;c++)std::memset(b->mBuffers[c].mData,0,frames*sizeof(float));return noErr;}
static void compare(const float* l,const float* r,std::vector<float>&expected,int frames,double&peak){
  for(int i=0;i<frames;i++)for(int c=0;c<2;c++){
    float actual=c?r[i]:l[i],want=expected[i*2+c];REQUIRE(std::isfinite(actual));
    if(std::abs(actual-want)>0.000002f){std::fprintf(stderr,"sample %d/%d actual %.9g reference %.9g\n",i,c,actual,want);REQUIRE(false);}
    peak=std::max(peak,std::abs(double(actual)));
  }
}
static void au(const char* path){
  CFURLRef url=CFURLCreateFromFileSystemRepresentation(nullptr,reinterpret_cast<const UInt8*>(path),std::strlen(path),true);
  CFBundleRef bundle=CFBundleCreate(nullptr,url);REQUIRE(bundle);REQUIRE(CFBundleLoadExecutable(bundle));
  auto factory=reinterpret_cast<AudioComponentFactoryFunction>(CFBundleGetFunctionPointerForName(bundle,CFSTR("LXR02_Factory")));REQUIRE(factory);
  AudioComponentDescription desc{kAudioUnitType_MusicDevice,'LxR2','ErRs',0,0};
  auto component=AudioComponentRegister(&desc,CFSTR("Erika Rust: LXR02"),0x300,factory);REQUIRE(component);
  for(double rate:{44100.,48000.,96000.,192000.}){
    AudioUnit unit=nullptr;REQUIRE(AudioComponentInstanceNew(component,&unit)==noErr);
    AudioStreamBasicDescription f{rate,kAudioFormatLinearPCM,kAudioFormatFlagIsFloat|kAudioFormatFlagIsNonInterleaved|kAudioFormatFlagsNativeEndian,4,1,4,2,32,0};

    REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_StreamFormat,kAudioUnitScope_Output,0,&f,sizeof(f))==noErr);
    // A valid AU host can use the default buffer limit without setting it.
    if(rate!=48000.){UInt32 max=1024;REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_MaximumFramesPerSlice,kAudioUnitScope_Global,0,&max,sizeof(max))==noErr);}
    Transport t;HostCallbackInfo callbacks{};callbacks.hostUserData=&t;callbacks.beatAndTempoProc=beatTempo;callbacks.transportStateProc=transport;
    REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_HostCallbacks,kAudioUnitScope_Global,0,&callbacks,sizeof(callbacks))==noErr);

    REQUIRE(AudioUnitInitialize(unit)==noErr);
    UInt32 maximumFrames=0,maximumSize=sizeof(maximumFrames);
    REQUIRE(AudioUnitGetProperty(unit,kAudioUnitProperty_MaximumFramesPerSlice,kAudioUnitScope_Global,0,&maximumFrames,&maximumSize)==noErr);
    REQUIRE(maximumFrames>0);
    void* reference=lxrp_create(rate);REQUIRE(reference);
    for(int p=0;p<283;p++){AudioUnitParameterValue v;REQUIRE(AudioUnitGetParameter(unit,p,kAudioUnitScope_Global,0,&v)==noErr);lxrp_set(reference,p,v);}
    Float64 latency=0.;UInt32 size=sizeof(latency);REQUIRE(AudioUnitGetProperty(unit,kAudioUnitProperty_Latency,kAudioUnitScope_Global,0,&latency,&size)==noErr);
    std::fprintf(stderr,"AU rate %.0f latency %.6f samples\n",rate,latency*rate);
    REQUIRE(std::abs(latency*rate-(int)std::lround(15.5*rate/44140.625))<0.01);
    std::array<float,1024> l{},r{};struct Stereo{UInt32 count;AudioBuffer b[2];}data{2,{{1,4096,l.data()},{1,4096,r.data()}}};
    std::vector<float> expected(2048);double peak=0.;int stamp=0;
    for(int phase=0;phase<6;phase++){
      t.bpm=phase==2?160.:phase==3?90.:120.;t.beat=phase==3?0.125:phase*.75;t.playing=phase!=4;
      for(int requested:{1,17,128,511,1024}){
        int n=std::min(requested,static_cast<int>(maximumFrames));
        lxrp_transport(reference,t.bpm,t.beat,t.playing);
        int eventOffset=n/2;int note=24+phase;
        if(n==128){REQUIRE(MusicDeviceMIDIEvent(unit,0x90,note,100,eventOffset)==noErr);lxrp_process(reference,expected.data(),eventOffset);lxrp_midi(reference,0x90,note,100);lxrp_process(reference,expected.data()+eventOffset*2,n-eventOffset);}
        else lxrp_process(reference,expected.data(),n);
        AudioTimeStamp time{};time.mFlags=kAudioTimeStampSampleTimeValid;time.mSampleTime=stamp;AudioUnitRenderActionFlags flags=0;
        data.b[0].mDataByteSize=data.b[1].mDataByteSize=n*4;
        REQUIRE(AudioUnitRender(unit,&flags,&time,0,n,reinterpret_cast<AudioBufferList*>(&data))==noErr);
        compare(l.data(),r.data(),expected,n,peak);stamp+=n;t.sample+=n;t.beat+=n/rate*t.bpm/60.;
      }
    }
    REQUIRE(peak>0.001);
    CFPropertyListRef saved=nullptr;size=sizeof(saved);REQUIRE(AudioUnitGetProperty(unit,kAudioUnitProperty_ClassInfo,kAudioUnitScope_Global,0,&saved,&size)==noErr);
    REQUIRE(AudioUnitSetParameter(unit,273,kAudioUnitScope_Global,0,70.,0)==noErr);
    REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_ClassInfo,kAudioUnitScope_Global,0,&saved,sizeof(saved))==noErr);
    AudioUnitParameterValue restored;REQUIRE(AudioUnitGetParameter(unit,273,kAudioUnitScope_Global,0,&restored)==noErr);REQUIRE(std::abs(restored)<.00001);
    CFRelease(saved);lxrp_destroy(reference);AudioUnitUninitialize(unit);AudioComponentInstanceDispose(unit);
    std::printf("PASS AU %.0f Hz: MIDI offsets, overlap/fallback, transport, tempo, seek/loop, stop, variable blocks, latency, state; peak %.6f\n",rate,peak);
  }
  CFRelease(url);CFRelease(bundle);
}
struct Events{clap_input_events_t api;std::vector<clap_event_midi_t> data;
 Events(){api.ctx=this;api.size=[](const clap_input_events_t*p)->uint32_t{return static_cast<Events*>(p->ctx)->data.size();};api.get=[](const clap_input_events_t*p,uint32_t i)->const clap_event_header_t*{return &static_cast<Events*>(p->ctx)->data[i].header;};}
 void add(int time,int status,int key,int value){clap_event_midi_t e{};e.header.size=sizeof(e);e.header.time=time;e.header.type=CLAP_EVENT_MIDI;e.data[0]=status;e.data[1]=key;e.data[2]=value;data.push_back(e);}
};
static void clap(const char* path){
  void* library=dlopen(path,RTLD_NOW|RTLD_LOCAL);if(!library)std::fprintf(stderr,"%s\n",dlerror());REQUIRE(library);
  auto entry=static_cast<const clap_plugin_entry_t*>(dlsym(library,"clap_entry"));REQUIRE(entry&&entry->init(path));
  auto factory=static_cast<const clap_plugin_factory_t*>(entry->get_factory(CLAP_PLUGIN_FACTORY_ID));REQUIRE(factory&&factory->get_plugin_count(factory)==1);
  clap_host_t host{};host.clap_version=CLAP_VERSION;host.name="LXR02 transport tests";host.vendor="Local";host.version="1";host.url="https://github.com/free-audio/clap";
  host.get_extension=[](const clap_host_t*,const char*)->const void*{return nullptr;};host.request_restart=host.request_process=host.request_callback=[](const clap_host_t*){};
  for(double rate:{44100.,48000.,96000.,192000.}){
    auto plugin=factory->create_plugin(factory,&host,factory->get_plugin_descriptor(factory,0)->id);REQUIRE(plugin&&plugin->init(plugin));
    auto params=static_cast<const clap_plugin_params_t*>(plugin->get_extension(plugin,CLAP_EXT_PARAMS));REQUIRE(params&&params->count(plugin)==283);
    clap_output_events_t outEvents{};outEvents.try_push=[](const clap_output_events_t*,const clap_event_header_t*){return true;};

    REQUIRE(plugin->activate(plugin,rate,1,1024));REQUIRE(plugin->start_processing(plugin));
    void* reference=lxrp_create(rate);for(int id=0;id<283;id++){double value=0.;REQUIRE(params->get_value(plugin,id,&value));char name[64];float b[3];lxrp_info(id,name,sizeof(name),b);if(!b[2])value=b[0]+value*(b[1]-b[0]);lxrp_set(reference,id,value);}
    auto latency=static_cast<const clap_plugin_latency_t*>(plugin->get_extension(plugin,CLAP_EXT_LATENCY));REQUIRE(latency&&latency->get(plugin)==(int)std::lround(15.5*rate/44140.625));
    std::array<float,1024> l{},r{},zero{};float* inputs[2]={zero.data(),zero.data()};float* outputs[2]={l.data(),r.data()};
    clap_audio_buffer_t inputBuffer{},outputBuffer{};inputBuffer.data32=inputs;inputBuffer.channel_count=2;outputBuffer.data32=outputs;outputBuffer.channel_count=2;
    Events empty;std::vector<float> expected(2048);double peak=0.;int64_t sample=0;
    for(int phase=0;phase<6;phase++){
      double bpm=phase==2?160.:phase==3?90.:120.,beat=phase==3?.125:phase*.75;bool playing=phase!=4;
      for(int n:{1,17,128,511,1024}){
        clap_event_transport_t transport{};transport.header.size=sizeof(transport);transport.header.type=CLAP_EVENT_TRANSPORT;
        transport.flags=CLAP_TRANSPORT_HAS_TEMPO|CLAP_TRANSPORT_HAS_BEATS_TIMELINE|CLAP_TRANSPORT_HAS_SECONDS_TIMELINE|(playing?CLAP_TRANSPORT_IS_PLAYING:0);
        transport.tempo=bpm;transport.song_pos_beats=std::llround(beat*CLAP_BEATTIME_FACTOR);transport.song_pos_seconds=std::llround(sample/rate*CLAP_SECTIME_FACTOR);
        clap_process_t process{};process.steady_time=sample;process.frames_count=n;process.transport=&transport;process.audio_inputs=nullptr;process.audio_outputs=&outputBuffer;process.audio_inputs_count=0;process.audio_outputs_count=1;process.in_events=&empty.api;process.out_events=&outEvents;
        lxrp_transport(reference,bpm,double(transport.song_pos_beats)/CLAP_BEATTIME_FACTOR,playing);
        Events events;
        if(n==128){events.add(7,0x90,24+phase,100);events.add(31,0x91,31-phase,100);events.add(63,0x81,31-phase,0);}
        if(n==511&&phase==1)events.add(300,0xb0,123,0);
        process.in_events=&events.api;int at=0;
        for(const auto&e:events.data){lxrp_process(reference,expected.data()+at*2,e.header.time-at);at=e.header.time;lxrp_midi(reference,e.data[0],e.data[1],e.data[2]);}
        lxrp_process(reference,expected.data()+at*2,n-at);
        REQUIRE(plugin->process(plugin,&process)!=CLAP_PROCESS_ERROR);compare(l.data(),r.data(),expected,n,peak);sample+=n;beat+=n/rate*bpm/60.;
      }
    }
    REQUIRE(peak>0.001);plugin->stop_processing(plugin);plugin->deactivate(plugin);
    auto state=static_cast<const clap_plugin_state_t*>(plugin->get_extension(plugin,CLAP_EXT_STATE));REQUIRE(state);
    std::vector<uint8_t> bytes;clap_ostream_t output{};output.ctx=&bytes;output.write=[](const clap_ostream_t*p,const void*b,uint64_t size)->int64_t{auto&v=*static_cast<std::vector<uint8_t>*>(p->ctx);v.insert(v.end(),static_cast<const uint8_t*>(b),static_cast<const uint8_t*>(b)+size);return size;};REQUIRE(state->save(plugin,&output));
    clap_event_param_value_t change{};change.header.size=sizeof(change);change.header.type=CLAP_EVENT_PARAM_VALUE;change.param_id=273;change.value=70.;change.note_id=-1;change.port_index=change.channel=change.key=-1;
    clap_input_events_t changeEvents{};changeEvents.ctx=&change;changeEvents.size=[](const clap_input_events_t*)->uint32_t{return 1;};changeEvents.get=[](const clap_input_events_t*p,uint32_t)->const clap_event_header_t*{return &static_cast<clap_event_param_value_t*>(p->ctx)->header;};params->flush(plugin,&changeEvents,&outEvents);
    struct Reader{std::vector<uint8_t>*bytes;size_t offset=0;}reader{&bytes};clap_istream_t input{};input.ctx=&reader;input.read=[](const clap_istream_t*p,void*b,uint64_t size)->int64_t{auto&r=*static_cast<Reader*>(p->ctx);size=std::min<uint64_t>(size,r.bytes->size()-r.offset);std::memcpy(b,r.bytes->data()+r.offset,size);r.offset+=size;return size;};REQUIRE(state->load(plugin,&input));
    double restored=0.;REQUIRE(params->get_value(plugin,273,&restored)&&std::abs(restored)<.00001);
    lxrp_destroy(reference);plugin->destroy(plugin);std::printf("PASS CLAP %.0f Hz: MIDI offsets, overlap/fallback, transport, tempo, seek/loop, stop, variable blocks, latency, state; peak %.6f\n",rate,peak);
  }
  entry->deinit();dlclose(library);
}
int main(int argc,char**argv){REQUIRE(argc==3);au(argv[1]);clap(argv[2]);std::puts("PASS native format host tests");return 0;}
