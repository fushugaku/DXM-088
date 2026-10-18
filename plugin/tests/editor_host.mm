// Native Cocoa CLAP host for editor/bridge QA. It processes silently: no device permissions.
#import <Cocoa/Cocoa.h>
#include <clap/clap.h>
#include <dlfcn.h>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstring>

static const clap_plugin_t* plug;
static const clap_plugin_gui_t* gui;
static const clap_plugin_entry_t* entry;
static NSWindow* window;
static bool callbackPending=false;
static int64_t frame=0;
static double bpm=120.;
static clap_host_gui_t hostGUI {
  [](const clap_host_t*){},
  [](const clap_host_t*,uint32_t w,uint32_t h){[window setContentSize:NSMakeSize(w,h)];return true;},
  [](const clap_host_t*){[window makeKeyAndOrderFront:nil];return true;},
  [](const clap_host_t*){[window orderOut:nil];return true;},
  [](const clap_host_t*,bool){}
};
static const void* extension(const clap_host_t*,const char* id) { return !std::strcmp(id,CLAP_EXT_GUI)?&hostGUI:nullptr; }
static void request(const clap_host_t*) {}
static void callback(const clap_host_t*) { callbackPending=true; }
static clap_host_t host {CLAP_VERSION,nullptr,"LXR02 editor test","ErikaLab","","0.2.0",extension,request,request,callback};
static clap_input_events_t noInput {nullptr,[](const clap_input_events_t*)->uint32_t{return 0;},[](const clap_input_events_t*,uint32_t)->const clap_event_header_t*{return nullptr;}};
static clap_output_events_t noOutput {nullptr,[](const clap_output_events_t*,const clap_event_header_t*){return true;}};

static void tick() {
  if(callbackPending){callbackPending=false;plug->on_main_thread(plug);}
  std::array<float,512> l{},r{},a{},b{};float* in[]={l.data(),r.data()};float* out[]={a.data(),b.data()};
  clap_audio_buffer_t input {in,nullptr,2,0,0},output {out,nullptr,2,0,0};
  clap_event_transport_t transport {};transport.header={sizeof(transport),0,0,CLAP_EVENT_TRANSPORT,0};transport.flags=CLAP_TRANSPORT_HAS_TEMPO|CLAP_TRANSPORT_HAS_BEATS_TIMELINE|CLAP_TRANSPORT_IS_PLAYING;transport.tempo=bpm;transport.tsig_num=transport.tsig_denom=4;
  clap_process_t process {};process.audio_inputs=nullptr;process.audio_outputs=&output;process.audio_inputs_count=0;process.audio_outputs_count=1;process.in_events=&noInput;process.out_events=&noOutput;process.transport=&transport;
  for(int remaining=1600;remaining>0;){const int n=std::min(512,remaining);
    static int last=-1;int note=24+((frame/192000)%8);clap_event_midi_t messages[2]{};uint32_t count=0;
    auto add=[&](int status,int key){auto&e=messages[count++];e.header={sizeof(e),0,0,CLAP_EVENT_MIDI,0};e.data[0]=status;e.data[1]=key;e.data[2]=100;};
    if(note!=last){if(last>=0)add(0x80,last);add(0x90,note);last=note;}
    struct Input{clap_event_midi_t*messages;uint32_t count;}events{messages,count};clap_input_events_t inputEvents{};inputEvents.ctx=&events;
    inputEvents.size=[](const clap_input_events_t*p)->uint32_t{return static_cast<Input*>(p->ctx)->count;};inputEvents.get=[](const clap_input_events_t*p,uint32_t at)->const clap_event_header_t*{return &static_cast<Input*>(p->ctx)->messages[at].header;};process.in_events=&inputEvents;
    process.frames_count=n;process.steady_time=frame;transport.song_pos_beats=std::llround(frame/48000.*bpm/60.*CLAP_BEATTIME_FACTOR);
    plug->process(plug,&process);frame+=n;remaining-=n;
  }
}
@interface LXR02HostDelegate : NSObject <NSApplicationDelegate,NSWindowDelegate>
@end
@implementation LXR02HostDelegate
- (void)applicationDidFinishLaunching:(NSNotification*)notification {
  NSString* path=[[NSBundle mainBundle] objectForInfoDictionaryKey:@"LXR02PluginBinary"];
  void* library=dlopen(path.fileSystemRepresentation,RTLD_NOW|RTLD_LOCAL);
  if(!library){std::fprintf(stderr,"%s\n",dlerror());[NSApp terminate:nil];return;}
  entry=static_cast<const clap_plugin_entry_t*>(dlsym(library,"clap_entry"));
  if(!entry||!entry->init(path.fileSystemRepresentation)){[NSApp terminate:nil];return;}
  const auto* factory=static_cast<const clap_plugin_factory_t*>(entry->get_factory(CLAP_PLUGIN_FACTORY_ID));
  plug=factory->create_plugin(factory,&host,factory->get_plugin_descriptor(factory,0)->id);
  if(!plug||!plug->init(plug)||!plug->activate(plug,48000.,1,512)||!plug->start_processing(plug)){[NSApp terminate:nil];return;}
  gui=static_cast<const clap_plugin_gui_t*>(plug->get_extension(plug,CLAP_EXT_GUI));
  if(!gui||!gui->create(plug,CLAP_WINDOW_API_COCOA,false)){[NSApp terminate:nil];return;}
  uint32_t w=1120,h=860;gui->get_size(plug,&w,&h);
  window=[[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,w,h) styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskMiniaturizable|NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
  window.title=@"LXR02 Rust · CLAP editor test";window.contentMinSize=NSMakeSize(360,520);window.delegate=self;
  clap_window_t parent {};parent.api=CLAP_WINDOW_API_COCOA;parent.cocoa=(__bridge void*)window.contentView;gui->set_parent(plug,&parent);gui->show(plug);
  [window center];[window makeKeyAndOrderFront:nil];
  [NSTimer scheduledTimerWithTimeInterval:1./30. repeats:YES block:^(NSTimer*){tick();}];
}
- (void)windowDidResize:(NSNotification*)notification { if(gui&&window){NSSize s=window.contentView.bounds.size;gui->set_size(plug,s.width,s.height);} }
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication*)app { return YES; }
- (void)applicationWillTerminate:(NSNotification*)notification { if(plug){gui->destroy(plug);plug->stop_processing(plug);plug->deactivate(plug);plug->destroy(plug);entry->deinit();} }
@end
int main(){@autoreleasepool{NSApplication* app=[NSApplication sharedApplication];[app setActivationPolicy:NSApplicationActivationPolicyRegular];LXR02HostDelegate* delegate=[LXR02HostDelegate new];app.delegate=delegate;[app run];}return 0;}
