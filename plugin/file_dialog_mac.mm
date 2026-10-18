#import <Cocoa/Cocoa.h>
#include <cstring>
extern "C" bool lxr_save_project_dialog(const char* name,char* path,size_t capacity){
 @autoreleasepool {
  NSSavePanel* panel=[NSSavePanel savePanel];NSString* filename=[NSString stringWithUTF8String:name];NSString* ext=filename.pathExtension;panel.allowedFileTypes=ext.length?@[ext]:nil;panel.canCreateDirectories=YES;
  panel.nameFieldStringValue=[NSString stringWithUTF8String:name];panel.title=@"Save DXM-088 file";
  if([panel runModal]!=NSModalResponseOK)return false;
  const char* utf8=panel.URL.path.UTF8String;const size_t size=std::strlen(utf8);if(size>=capacity)return false;std::memcpy(path,utf8,size+1);return true;
 }
}

// Objective-C classes must have different names when a DAW loads several formats.
#if defined(AU_API)
#define LXRFileDragSource ErikaRustLXR02AUFileDragSource
#elif defined(VST3_API)
#define LXRFileDragSource ErikaRustLXR02VST3FileDragSource
#elif defined(CLAP_API)
#define LXRFileDragSource ErikaRustLXR02CLAPFileDragSource
#else
#define LXRFileDragSource ErikaRustLXR02StandaloneFileDragSource
#endif
// Cocoa file drags let DAWs import real files, including all seven stem files.
@interface LXRFileDragSource : NSObject<NSDraggingSource>
@end
static NSMutableSet* lxrActiveDrags;
@implementation LXRFileDragSource
- (NSDragOperation)draggingSession:(NSDraggingSession*)session sourceOperationMaskForDraggingContext:(NSDraggingContext)context { return NSDragOperationCopy; }
- (BOOL)ignoreModifierKeysForDraggingSession:(NSDraggingSession*)session { return YES; }
- (void)draggingSession:(NSDraggingSession*)session endedAtPoint:(NSPoint)point operation:(NSDragOperation)operation { [lxrActiveDrags removeObject:self]; }
@end
extern "C" bool lxr_prepare_export(const char* name,const char* encoded,char* path,size_t capacity){
 @autoreleasepool {
  NSString* filename=[[NSString stringWithUTF8String:name] lastPathComponent];
  if(!filename.length||![ @[@"wav",@"mid"] containsObject:filename.pathExtension.lowercaseString])return false;
  NSData* data=[[NSData alloc] initWithBase64EncodedString:[NSString stringWithUTF8String:encoded] options:0];if(!data||data.length>64000000)return false;
  NSString* folder=[NSTemporaryDirectory() stringByAppendingPathComponent:[@"DXM-088-Exports/" stringByAppendingString:NSUUID.UUID.UUIDString]];
  if(![[NSFileManager defaultManager] createDirectoryAtPath:folder withIntermediateDirectories:YES attributes:nil error:nil])return false;
  NSString* file=[folder stringByAppendingPathComponent:filename];if(![data writeToFile:file options:NSDataWritingAtomic error:nil])return false;
  if(file.length*4+1>capacity)return false;const char* utf8=file.UTF8String;if(std::strlen(utf8)>=capacity)return false;std::memcpy(path,utf8,std::strlen(utf8)+1);return true;
 }
}
extern "C" bool lxr_drag_exports(void* rawView,const char* pathsJSON){
 @autoreleasepool {
  NSView* view=(__bridge NSView*)rawView;if(!view.window)return false;
  NSData* json=[[NSString stringWithUTF8String:pathsJSON] dataUsingEncoding:NSUTF8StringEncoding];NSArray* paths=[NSJSONSerialization JSONObjectWithData:json options:0 error:nil];if(![paths isKindOfClass:NSArray.class]||!paths.count)return false;
  NSMutableArray* items=[NSMutableArray array];NSPoint point=[view convertPoint:[view.window mouseLocationOutsideOfEventStream] fromView:nil];
  for(NSString* path in paths){NSURL* url=[NSURL fileURLWithPath:path];if(![url checkResourceIsReachableAndReturnError:nil])return false;NSDraggingItem* item=[[NSDraggingItem alloc] initWithPasteboardWriter:url];NSImage* icon=[[NSWorkspace sharedWorkspace] iconForFile:path];[item setDraggingFrame:NSMakeRect(point.x,point.y,40,40) contents:icon];[items addObject:item];}
  NSEvent* event=NSApp.currentEvent;
  if(!event||(event.type!=NSEventTypeLeftMouseDown&&event.type!=NSEventTypeLeftMouseDragged)){
    if(!(NSEvent.pressedMouseButtons&1))return false;
    event=[NSEvent mouseEventWithType:NSEventTypeLeftMouseDragged location:view.window.mouseLocationOutsideOfEventStream modifierFlags:NSEvent.modifierFlags timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:view.window.windowNumber context:nil eventNumber:0 clickCount:1 pressure:1.];
  }
  LXRFileDragSource* source=[LXRFileDragSource new];if(!lxrActiveDrags)lxrActiveDrags=[NSMutableSet new];[lxrActiveDrags addObject:source];
  [view beginDraggingSessionWithItems:items event:event source:source];return true;
 }
}
