#pragma once
#include <filesystem>
#include <fstream>
#include <cstdlib>
#include <algorithm>
#include <chrono>
#include <random>
#include <stdexcept>
#include "json.hpp"
#ifdef OS_WIN
#include <windows.h>
#endif

// Shared by AU, VST3 and CLAP. File I/O runs on the editor thread only.
namespace dxm {
class KitStore {
  std::filesystem::path mRoot;
  static constexpr uintmax_t kMaximum=24000000;
  static void validID(const std::string& id){if(id.empty()||id.size()>64||!std::all_of(id.begin(),id.end(),[](unsigned char c){return (c>='a'&&c<='z')||(c>='A'&&c<='Z')||(c>='0'&&c<='9')||c=='-';}))throw std::runtime_error("Invalid kit identifier.");}
  static std::filesystem::path root(){
#ifdef OS_WIN
    // Wide paths preserve non-ASCII Windows account names.
    if(const wchar_t* folder=_wgetenv(L"APPDATA"))return std::filesystem::path(folder)/L"DXM-088"/L"Kits";
#elif defined(OS_MAC) || defined(__APPLE__)
    if(const char* folder=std::getenv("HOME"))return std::filesystem::path(folder)/"Library"/"Application Support"/"DXM-088"/"Kits";
#else
    if(const char* folder=std::getenv("XDG_DATA_HOME"))return std::filesystem::path(folder)/"DXM-088"/"Kits";
    if(const char* folder=std::getenv("HOME"))return std::filesystem::path(folder)/".local"/"share"/"DXM-088"/"Kits";
#endif
    throw std::runtime_error("User kit folder is unavailable.");
  }
public:
  KitStore():mRoot(root()){}
  explicit KitStore(std::filesystem::path path):mRoot(std::move(path)){}
  nlohmann::json get(const std::string& id)const{
    validID(id);const auto path=mRoot/(id+".dxmkit");
    if(!std::filesystem::is_regular_file(path)||std::filesystem::file_size(path)>kMaximum)throw std::runtime_error("Kit is unavailable.");
    std::ifstream file(path,std::ios::binary);auto kit=nlohmann::json::parse(file,nullptr,false);
    if(kit.is_discarded()||kit.value("format","")!="dxm088-kit"||kit.value("version",0)!=1||kit.value("id","")!=id||!kit.contains("sound"))throw std::runtime_error("Kit could not be read.");
    return kit;
  }
  nlohmann::json list()const{
    auto result=nlohmann::json::array();if(!std::filesystem::exists(mRoot))return result;
    for(const auto& entry:std::filesystem::directory_iterator(mRoot))if(entry.is_regular_file()&&entry.path().extension()==".dxmkit"){
      try{const auto kit=get(entry.path().stem().u8string());result.push_back({{"id",kit.at("id")},{"name",kit.at("name")},{"updatedAt",kit.value("updatedAt","")}});}catch(const std::exception&){}
    }
    std::sort(result.begin(),result.end(),[](const auto& a,const auto& b){return a.at("name").template get<std::string>()<b.at("name").template get<std::string>();});return result;
  }
  void save(const nlohmann::json& kit)const{
    const auto id=kit.at("id").get<std::string>();validID(id);
    if(kit.value("format","")!="dxm088-kit"||kit.value("version",0)!=1||!kit.contains("sound")||!kit["sound"].is_object()||!kit["name"].is_string()||kit["name"].get_ref<const std::string&>().empty()||kit["name"].get_ref<const std::string&>().size()>240)throw std::runtime_error("Invalid kit.");
    const auto data=kit.dump();if(data.size()>kMaximum)throw std::runtime_error("Kit is too large.");
    std::filesystem::create_directories(mRoot);
    const auto path=mRoot/(id+".dxmkit"),temporary=mRoot/(id+"-"+std::to_string(std::chrono::steady_clock::now().time_since_epoch().count())+"-"+std::to_string(std::random_device{}())+".tmp");
    try{
      {std::ofstream file(temporary,std::ios::binary|std::ios::trunc);file.exceptions(std::ios::badbit|std::ios::failbit);file.write(data.data(),data.size());file.close();}
#ifdef OS_WIN
      if(!MoveFileExW(temporary.c_str(),path.c_str(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH))throw std::runtime_error("Kit could not be saved.");
#else
      std::filesystem::rename(temporary,path);
#endif
    }catch(...){std::error_code ignored;std::filesystem::remove(temporary,ignored);throw;}
  }
};
}
