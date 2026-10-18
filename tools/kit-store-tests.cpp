#include "../plugin/KitStore.h"
#include <iostream>
#define REQUIRE(x) do { if(!(x)){std::cerr<<"FAIL line "<<__LINE__<<": "<<#x<<"\n";return 1;} } while(false)
int main(){
  const auto root=std::filesystem::temp_directory_path()/std::filesystem::u8path(std::string(u8"dxm088-киты-")+std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  dxm::KitStore au(root),vst(root);REQUIRE(au.list().empty());
  nlohmann::json kit={{"format","dxm088-kit"},{"version",1},{"id","test-one"},{"name","Мой кит"},{"updatedAt","2026-10-18"},{"sound",{{"sample","layer"}}}};
  au.save(kit);REQUIRE(vst.list().size()==1);REQUIRE(vst.get("test-one")==kit);
  kit["sound"]["sample"]="changed";vst.save(kit);REQUIRE(au.get("test-one")==kit);REQUIRE(au.list().size()==1);
  kit["id"]="test-two";au.save(kit);REQUIRE(vst.list().size()==2);
  {std::ofstream corrupt(root/"corrupt.dxmkit");corrupt<<"broken";}REQUIRE(au.list().size()==2);
  bool rejected=false;try{kit["id"]="../escape";au.save(kit);}catch(const std::exception&){rejected=true;}REQUIRE(rejected);
  for(const auto& entry:std::filesystem::directory_iterator(root))REQUIRE(entry.path().extension()!=".tmp");
  std::filesystem::remove_all(root);std::cout<<"PASS shared user kit library: save, update, new kit, Unicode names, cross-instance reload, atomic replacement\n";
}
