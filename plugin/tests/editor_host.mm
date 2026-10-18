// Native Cocoa CLAP host for editor/bridge QA. It processes silently: no device permissions.
#import <Cocoa/Cocoa.h>
#include <clap/clap.h>
#include <dlfcn.h>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <vector>

static const clap_plugin_t* plug;
static const clap_plugin_gui_t* gui;
static const clap_plugin_entry_t* entry;
static NSWindow* window;
static NSView* editorView;
static NSStackView* toolbar;
static const clap_plugin_factory_t* factory;
static int requestedNote=-1,lastNote=-1;
static bool callbackPending=false;
static int64_t frame=0;
static double bpm=120.;
static clap_host_gui_t hostGUI {
  [](const clap_host_t*){},
  [](const clap_host_t*,uint32_t w,uint32_t h){[window setContentSize:NSMakeSize(w,h+40)];return true;},
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
    int note=requestedNote;clap_event_midi_t messages[2]{};uint32_t count=0;
    auto add=[&](int status,int key){auto&e=messages[count++];e.header={sizeof(e),0,0,CLAP_EVENT_MIDI,0};e.data[0]=status;e.data[1]=key;e.data[2]=100;};
    if(note!=lastNote){if(lastNote>=0)add(0x80,lastNote);if(note>=0)add(0x90,note);lastNote=note;}
    struct Input{clap_event_midi_t*messages;uint32_t count;}events{messages,count};clap_input_events_t inputEvents{};inputEvents.ctx=&events;
    inputEvents.size=[](const clap_input_events_t*p)->uint32_t{return static_cast<Input*>(p->ctx)->count;};inputEvents.get=[](const clap_input_events_t*p,uint32_t at)->const clap_event_header_t*{return &static_cast<Input*>(p->ctx)->messages[at].header;};process.in_events=&inputEvents;
    process.frames_count=n;process.steady_time=frame;transport.song_pos_beats=std::llround(frame/48000.*bpm/60.*CLAP_BEATTIME_FACTOR);
    plug->process(plug,&process);frame+=n;remaining-=n;
  }
}
static bool attachEditor(){
  gui=static_cast<const clap_plugin_gui_t*>(plug->get_extension(plug,CLAP_EXT_GUI));
  if(!gui||!gui->create(plug,CLAP_WINDOW_API_COCOA,false))return false;
  clap_window_t parent{};parent.api=CLAP_WINDOW_API_COCOA;parent.cocoa=(__bridge void*)editorView;
  if(!gui->set_parent(plug,&parent))return false;NSSize size=editorView.bounds.size;gui->set_size(plug,size.width,size.height);gui->show(plug);return true;
}
static std::vector<uint8_t> saveState(){
  auto state=static_cast<const clap_plugin_state_t*>(plug->get_extension(plug,CLAP_EXT_STATE));std::vector<uint8_t> bytes;
  clap_ostream_t stream{};stream.ctx=&bytes;stream.write=[](const clap_ostream_t*s,const void*p,uint64_t size)->int64_t{auto&v=*static_cast<std::vector<uint8_t>*>(s->ctx);v.insert(v.end(),static_cast<const uint8_t*>(p),static_cast<const uint8_t*>(p)+size);return size;};
  if(!state||!state->save(plug,&stream))return {};
  NSString* folder=[[NSBundle mainBundle]objectForInfoDictionaryKey:@"LXR02QAFolder"];
  NSString* path=[folder stringByAppendingPathComponent:@"editor-memory-state.clapstate"];
  [[NSData dataWithBytes:bytes.data() length:bytes.size()]writeToFile:path atomically:YES];return bytes;
}
@interface LXR02HostDelegate : NSObject <NSApplicationDelegate,NSWindowDelegate>
- (void)note36:(id)sender;
- (void)note45:(id)sender;
- (void)noteOff:(id)sender;
- (void)capture:(id)sender;
- (void)reopen:(id)sender;
- (void)narrow:(id)sender;
@end
@implementation LXR02HostDelegate
- (void)applicationDidFinishLaunching:(NSNotification*)notification {
  NSString* path=[[NSBundle mainBundle] objectForInfoDictionaryKey:@"LXR02PluginBinary"];
  void* library=dlopen(path.fileSystemRepresentation,RTLD_NOW|RTLD_LOCAL);
  if(!library){std::fprintf(stderr,"%s\n",dlerror());[NSApp terminate:nil];return;}
  entry=static_cast<const clap_plugin_entry_t*>(dlsym(library,"clap_entry"));
  if(!entry||!entry->init(path.fileSystemRepresentation)){[NSApp terminate:nil];return;}
  factory=static_cast<const clap_plugin_factory_t*>(entry->get_factory(CLAP_PLUGIN_FACTORY_ID));
  plug=factory->create_plugin(factory,&host,factory->get_plugin_descriptor(factory,0)->id);
  if(!plug||!plug->init(plug)||!plug->activate(plug,48000.,1,512)||!plug->start_processing(plug)){[NSApp terminate:nil];return;}
  const int w=1120,h=860;
  window=[[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,w,h+40) styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskMiniaturizable|NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
  window.title=@"DXM-088 · kit and pattern memory test";window.contentMinSize=NSMakeSize(360,520);window.delegate=self;
  editorView=[[NSView alloc]initWithFrame:NSMakeRect(0,0,w,h)];editorView.autoresizingMask=NSViewWidthSizable|NSViewHeightSizable;[window.contentView addSubview:editorView];
  toolbar=[[NSStackView alloc]initWithFrame:NSMakeRect(8,h,w-16,40)];toolbar.orientation=NSUserInterfaceLayoutOrientationHorizontal;toolbar.spacing=8;toolbar.detachesHiddenViews=YES;toolbar.autoresizingMask=NSViewWidthSizable|NSViewMinYMargin;
  NSArray* names=@[@"MIDI 36",@"MIDI 45",@"Note off",@"Capture state",@"Save & reopen",@"390 px"];
  SEL actions[]={@selector(note36:),@selector(note45:),@selector(noteOff:),@selector(capture:),@selector(reopen:),@selector(narrow:)};
  for(int i=0;i<6;i++){NSButton* button=[NSButton buttonWithTitle:names[i] target:self action:actions[i]];[toolbar addArrangedSubview:button];}[window.contentView addSubview:toolbar];
  if(!attachEditor()){[NSApp terminate:nil];return;}
  [window center];[window makeKeyAndOrderFront:nil];
  [NSTimer scheduledTimerWithTimeInterval:1./30. repeats:YES block:^(NSTimer*){tick();}];
}
- (void)note36:(id)sender{requestedNote=36;}
- (void)note45:(id)sender{requestedNote=45;}
- (void)noteOff:(id)sender{requestedNote=-1;}
- (void)capture:(id)sender{saveState();}
- (void)narrow:(id)sender{const bool small=window.contentView.bounds.size.width>=500;for(NSButton* button in toolbar.arrangedSubviews)if(![button.title isEqualToString:@"390 px"])button.hidden=small;[toolbar layoutSubtreeIfNeeded];[window setContentSize:NSMakeSize(small?390:1120,900)];}
- (void)reopen:(id)sender{
  auto bytes=saveState();if(bytes.empty())return;gui->destroy(plug);plug->stop_processing(plug);plug->deactivate(plug);plug->destroy(plug);callbackPending=false;
  plug=factory->create_plugin(factory,&host,factory->get_plugin_descriptor(factory,0)->id);if(!plug||!plug->init(plug)){[NSApp terminate:nil];return;}
  struct Reader{std::vector<uint8_t>*bytes;size_t at=0;} reader{&bytes};clap_istream_t stream{};stream.ctx=&reader;stream.read=[](const clap_istream_t*s,void*p,uint64_t n)->int64_t{auto&r=*static_cast<Reader*>(s->ctx);n=std::min<uint64_t>(n,r.bytes->size()-r.at);std::memcpy(p,r.bytes->data()+r.at,n);r.at+=n;return n;};
  auto state=static_cast<const clap_plugin_state_t*>(plug->get_extension(plug,CLAP_EXT_STATE));if(!state||!state->load(plug,&stream)||!plug->activate(plug,48000.,1,512)||!plug->start_processing(plug)||!attachEditor()){[NSApp terminate:nil];return;}lastNote=-1;
}
- (void)windowDidResize:(NSNotification*)notification { if(gui&&editorView){NSSize s=editorView.bounds.size;gui->set_size(plug,s.width,s.height);} }
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication*)app { return YES; }
- (void)applicationWillTerminate:(NSNotification*)notification { if(plug){gui->destroy(plug);plug->stop_processing(plug);plug->deactivate(plug);plug->destroy(plug);entry->deinit();} }
@end
int main(){@autoreleasepool{NSApplication* app=[NSApplication sharedApplication];[app setActivationPolicy:NSApplicationActivationPolicyRegular];LXR02HostDelegate* delegate=[LXR02HostDelegate new];app.delegate=delegate;[app run];}return 0;}
