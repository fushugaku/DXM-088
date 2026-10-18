#include "../plugin/KitStore.h"
#include <cassert>
#include <iostream>
int main(){
  const auto root=std::filesystem::temp_directory_path()/("dxm088-kits-"+std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  dxm::KitStore au(root),vst(root);assert(au.list().empty());
  nlohmann::json kit={{"format","dxm088-kit"},{"version",1},{"id","test-one"},{"name","Мой кит"},{"updatedAt","2026-10-18"},{"sound",{{"sample","layer"}}}};
  au.save(kit);assert(vst.list().size()==1);assert(vst.get("test-one")==kit);
  kit["sound"]["sample"]="changed";vst.save(kit);assert(au.get("test-one")==kit);assert(au.list().size()==1);
  kit["id"]="test-two";au.save(kit);assert(vst.list().size()==2);
  {std::ofstream corrupt(root/"corrupt.dxmkit");corrupt<<"broken";}assert(au.list().size()==2);
  bool rejected=false;try{kit["id"]="../escape";au.save(kit);}catch(const std::exception&){rejected=true;}assert(rejected);
  for(const auto& entry:std::filesystem::directory_iterator(root))assert(entry.path().extension()!=".tmp");
  std::filesystem::remove_all(root);std::cout<<"PASS shared user kit library: save, update, new kit, Unicode names, cross-instance reload, atomic replacement\n";
}
