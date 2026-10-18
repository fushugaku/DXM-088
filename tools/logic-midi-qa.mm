// Interactive CoreMIDI source for testing the installed plug-in in a real DAW.
// Build: clang++ -std=c++17 tools/logic-midi-qa.mm -framework CoreMIDI -framework CoreFoundation -o target/logic-midi-qa
// Commands: on NOTE [VELOCITY], off NOTE, quit. Disconnecting releases held notes.
#include <CoreMIDI/CoreMIDI.h>
#include <mach/mach_time.h>
#include <array>
#include <chrono>
#include <fstream>
#include <iostream>
#include <sstream>
#include <string>

int main(int argc, char** argv) {
  MIDIClientRef client = 0;
  MIDIEndpointRef source = 0;
  if (MIDIClientCreate(CFSTR("LXR02 Logic QA"), nullptr, nullptr, &client) ||
      MIDISourceCreate(client, CFSTR("LXR02 Logic QA"), &source)) return 1;
  std::array<bool,128> held{};
  std::ofstream log;
  if (argc > 1) log.open(argv[1], std::ios::app);
  auto send = [&](int status, int key, int value) {
    MIDIPacketList packets{};
    auto* packet = MIDIPacketListInit(&packets);
    const Byte bytes[] = {Byte(status), Byte(key), Byte(value)};
    if (!MIDIPacketListAdd(&packets, sizeof(packets), packet, mach_absolute_time(), 3, bytes)) return false;
    const auto result = MIDIReceived(source, &packets);
    const auto now = std::chrono::duration_cast<std::chrono::milliseconds>(
      std::chrono::system_clock::now().time_since_epoch()).count();
    std::ostringstream entry;
    entry << "{\"timeMs\":" << now << ",\"status\":" << status << ",\"note\":" << key
          << ",\"value\":" << value << ",\"result\":" << result << "}";
    std::cout << entry.str() << std::endl;
    if (log) { log << entry.str() << '\n'; log.flush(); }
    return result == noErr;
  };
  std::cout << "Ready: LXR02 Logic QA. MIDI channel 1. on NOTE [VELOCITY] / off NOTE / quit" << std::endl;
  std::string line;
  while (std::getline(std::cin, line)) {
    std::istringstream input(line);
    std::string op; int key = -1, velocity = 100;
    input >> op >> key >> velocity;
    if (op == "quit") break;
    if ((op != "on" && op != "off") || key < 0 || key > 127 || velocity < 1 || velocity > 127) {
      std::cout << "Expected: on NOTE [1..127] / off NOTE / quit" << std::endl; continue;
    }
    // Avoid counting the same test key twice; the instrument itself supports overlapping Note Ons.
    if (op == "on" && held[key]) send(0x80, key, 0);
    if (!send(op == "on" ? 0x90 : 0x80, key, op == "on" ? velocity : 0)) break;
    held[key] = op == "on";
  }
  for (int key=0; key<128; ++key) if (held[key]) send(0x80, key, 0);
  MIDIEndpointDispose(source);
  MIDIClientDispose(client);
}
