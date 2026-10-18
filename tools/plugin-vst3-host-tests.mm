// Exercise the shipped VST3 binary with genuine SDK transport/parameter APIs.
#include <CoreFoundation/CoreFoundation.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
#include "pluginterfaces/base/ipluginbase.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "pluginterfaces/vst/ivstcomponent.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include "pluginterfaces/vst/ivstprocesscontext.h"
#include "public.sdk/source/common/memorystream.h"
#include "public.sdk/source/vst/hosting/hostclasses.h"
#include "public.sdk/source/vst/hosting/parameterchanges.h"
#include "public.sdk/source/vst/hosting/eventlist.h"
#include "../plugin/engine.h"

using namespace Steinberg;
using namespace Steinberg::Vst;
#define REQUIRE(x) do { if (!(x)) { std::fprintf(stderr,"FAIL line %d: %s\n",__LINE__,#x); std::exit(1); } } while (0)
static void change(ParameterChanges& changes, ParamID id, double value) {
  int32 index=0; auto queue=changes.addParameterData(id,index);
  REQUIRE(queue&&queue->addPoint(0,value,index)==kResultOk);
}
int main(int argc,char** argv) {
  REQUIRE(argc==2);
  auto url=CFURLCreateFromFileSystemRepresentation(nullptr,reinterpret_cast<const UInt8*>(argv[1]),std::strlen(argv[1]),true);
  auto bundle=CFBundleCreate(nullptr,url); REQUIRE(bundle&&CFBundleLoadExecutable(bundle));
  auto enter=reinterpret_cast<bool(*)(CFBundleRef)>(CFBundleGetFunctionPointerForName(bundle,CFSTR("bundleEntry")));
  auto leave=reinterpret_cast<bool(*)()>(CFBundleGetFunctionPointerForName(bundle,CFSTR("bundleExit")));
  auto getFactory=reinterpret_cast<IPluginFactory*(*)()>(CFBundleGetFunctionPointerForName(bundle,CFSTR("GetPluginFactory")));
  REQUIRE(enter&&leave&&getFactory&&enter(bundle));
  auto factory=owned(getFactory()); REQUIRE(factory);
  PClassInfo info{}; REQUIRE(factory->getClassInfo(0,&info)==kResultOk);
  HostApplication host;
  for(double rate:{44100.,48000.,96000.,192000.}) {
    IComponent* raw=nullptr;
    REQUIRE(factory->createInstance(info.cid,IComponent::iid,reinterpret_cast<void**>(&raw))==kResultOk);
    auto component=owned(raw); REQUIRE(component->initialize(&host)==kResultOk);
    FUnknownPtr<IAudioProcessor> processor(component); FUnknownPtr<IEditController> controller(component);
    REQUIRE(processor&&controller);
    SpeakerArrangement stereo=SpeakerArr::kStereo;
    REQUIRE(processor->setBusArrangements(nullptr,0,&stereo,1)==kResultOk);

    REQUIRE(component->activateBus(kAudio,kOutput,0,true)==kResultOk);
    ProcessSetup setup{kRealtime,kSample32,1024,rate};
    REQUIRE(processor->setupProcessing(setup)==kResultOk);
    REQUIRE(component->setActive(true)==kResultOk);
    REQUIRE(processor->setProcessing(true)==kResultOk);
    void* reference=lxrp_create(rate); REQUIRE(reference);
    ParameterChanges changes;
    for(int p=0;p<283;p++){char name[64];float b[3];lxrp_info(p,name,sizeof(name),b);double normalized=controller->getParamNormalized(p);lxrp_set(reference,p,b[0]+normalized*(b[1]-b[0]));}
    REQUIRE(processor->getLatencySamples()==(int)std::lround(15.5*rate/44140.625));
    std::array<float,1024> left{},right{},zero{};
    float* ins[2]={zero.data(),zero.data()};float* outs[2]={left.data(),right.data()};
    AudioBusBuffers input{},output{};input.numChannels=output.numChannels=2;input.channelBuffers32=ins;output.channelBuffers32=outs;
    std::vector<float> expected(2048);double peak=0.;int64 sample=0;
    for(int phase=0;phase<6;phase++) {
      double bpm=phase==2?160.:phase==3?90.:120.,beat=phase==3?.125:phase*.75;
      bool playing=phase!=4;
      for(int n:{1,17,128,511,1024}) {
        ProcessContext context{};context.state=ProcessContext::kTempoValid|ProcessContext::kProjectTimeMusicValid|(playing?ProcessContext::kPlaying:0);
        context.sampleRate=rate;context.projectTimeSamples=sample;context.projectTimeMusic=beat;context.tempo=bpm;
        ProcessData data{};data.processMode=kRealtime;data.symbolicSampleSize=kSample32;data.numSamples=n;data.numInputs=0;data.numOutputs=1;
        data.inputs=nullptr;data.outputs=&output;data.inputParameterChanges=&changes;data.processContext=&context;
        std::fill(expected.begin(),expected.end(),0.f);
        lxrp_transport(reference,bpm,beat,playing);EventList events;data.inputEvents=&events;
        if(n==128){Event e{};e.type=Event::kNoteOnEvent;e.sampleOffset=17;e.noteOn.channel=0;e.noteOn.pitch=24+phase;e.noteOn.velocity=100.f/127.f;e.noteOn.noteId=-1;REQUIRE(events.addEvent(e)==kResultOk);lxrp_process(reference,expected.data(),17);lxrp_midi(reference,0x90,24+phase,100);lxrp_process(reference,expected.data()+34,n-17);}
        else lxrp_process(reference,expected.data(),n);
        REQUIRE(processor->process(data)==kResultOk);
        for(int i=0;i<n;i++)for(int c=0;c<2;c++) {
          float actual=outs[c][i],want=expected[i*2+c];
          if(!std::isfinite(actual)||std::abs(actual-want)>0.000002f) {
            std::fprintf(stderr,"%.0f Hz sample %d/%d actual %.9g reference %.9g\n",rate,i,c,actual,want);REQUIRE(false);
          }
          peak=std::max(peak,std::abs(double(actual)));
        }
        changes.clearQueue(); sample+=n;beat+=n/rate*bpm/60.;
      }
    }
    REQUIRE(peak>0.001);
    REQUIRE(processor->setProcessing(false)==kResultOk); REQUIRE(component->setActive(false)==kResultOk);
    MemoryStream saved;REQUIRE(component->getState(&saved)==kResultOk);
    REQUIRE(controller->setParamNormalized(273,70./120.)==kResultOk);
    REQUIRE(saved.seek(0,IBStream::kIBSeekSet,nullptr)==kResultOk);
    REQUIRE(component->setState(&saved)==kResultOk);
    REQUIRE(std::abs(controller->getParamNormalized(273)-24./120.)<0.00001);
    lxrp_destroy(reference);REQUIRE(component->terminate()==kResultOk);
    std::printf("PASS VST3 %.0f Hz: note offsets, transport, tempo, seek/loop, stop, variable blocks, latency, state; peak %.6f\n",rate,peak);
  }
  factory=nullptr;REQUIRE(leave());CFRelease(bundle);CFRelease(url);
}
