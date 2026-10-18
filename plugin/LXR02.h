#pragma once
#include "IPlug_include_in_plug_hdr.h"
#include "engine.h"
#include <array>
#include <atomic>
#include <mutex>
#include <string>
#include <vector>

class LXR02 final:public iplug::Plugin {
public:
  explicit LXR02(const iplug::InstanceInfo&);
  ~LXR02() override;
  void ProcessBlock(iplug::sample**,iplug::sample**,int) override;
  void ProcessMidiMsg(const iplug::IMidiMsg&) override;
  void OnReset() override;
  void OnActivate(bool active) override {if(active)OnReset();}
  void OnIdle() override;
  void OnUIOpen() override;
  void CloseWindow() override;
  bool OnMessage(int,int,int,const void*) override;
#ifdef WEBVIEW_EDITOR_DELEGATE
  bool OnCanDownloadMIMEType(const char*) override;
  void OnGetLocalDownloadPathForFile(const char*,WDL_String&) override;
  void OnDownloadedFile(const char*) override;
#endif
  bool SerializeState(iplug::IByteChunk&) const override;
  int UnserializeState(const iplug::IByteChunk&,int) override;
private:
#ifdef CLAP_API
  const clap_host_t* mClapHost=nullptr;
  bool stateSave(const clap_ostream*) noexcept override;
  bool stateLoad(const clap_istream*) noexcept override;
  bool audioPortsGetConfig(uint32_t,clap_audio_ports_config*) const noexcept override;
  void reset() noexcept override;
  bool notePortsInfo(uint32_t,bool,clap_note_port_info*) const noexcept override;
#endif
  static constexpr int kParams=283;
  struct Ready {void* engine;uint64_t revision;};
  struct Command {uint64_t revision;uint32_t op;std::array<double,8> args{};};
  static constexpr uint32_t kQueue=2048;
  void PublishModel();
  void SyncModel() const;
  void SendProject();
  void NotifyParams();
  void EndUIGestures();
  std::string ProjectJSON() const;
  std::string mExportToken;
  std::vector<std::string> mExportPaths;
  void Retire(Ready*);
  mutable std::mutex mModelMutex;
  void* mModel=nullptr;
  Ready* mActive=nullptr;
  std::atomic<Ready*> mPending{nullptr},mRetired{nullptr};
  uint64_t mRevision=0;
  std::array<Command,kQueue> mCommands{};
  std::atomic<uint32_t> mWrite{0},mRead{0};
  std::array<iplug::IMidiMsg,4096> mMidi{};
  int mMidiCount=0;
  std::atomic<bool> mOverflow{false},mProjectDirty{false},mEditorReady{false};
  std::atomic<float> mRate{44100.f};
  std::array<float,8192> mAudio{};
  std::array<float,kParams> mParamCache{};
  std::array<float,kParams> mUISent{};
  std::array<bool,kParams> mUIGestures{};
  std::array<std::atomic<float>,20> mStatus{};
  std::array<std::atomic<float>,128> mScope{};
};
