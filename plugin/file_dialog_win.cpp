#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commdlg.h>
#include <cstddef>
extern "C" bool lxr_save_project_dialog(const char* name,char* path,size_t capacity){
 wchar_t file[32768]{};MultiByteToWideChar(CP_UTF8,0,name,-1,file,32768);
 OPENFILENAMEW dialog{};dialog.lStructSize=sizeof(dialog);dialog.lpstrFile=file;dialog.nMaxFile=32768;
 dialog.lpstrFilter=L"DXM-088 files\0*.json;*.wav;*.mid;*.zip\0\0";dialog.lpstrTitle=L"Save DXM-088 file";
 dialog.Flags=OFN_OVERWRITEPROMPT|OFN_PATHMUSTEXIST|OFN_NOCHANGEDIR;
 if(!GetSaveFileNameW(&dialog))return false;
 return WideCharToMultiByte(CP_UTF8,0,file,-1,path,(int)capacity,nullptr,nullptr)>0;
}
