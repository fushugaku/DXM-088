#pragma once
#include <cstddef>
#include <cstdint>
extern "C" {
void* lxrp_create(float);
void lxrp_destroy(void*);
void* lxrp_clone(void*,float);
void lxrp_inherit(void*,void*);
void lxrp_set(void*,uint32_t,float);
float lxrp_get(void*,uint32_t);
void lxrp_command(void*,uint32_t,const double*);
void lxrp_midi(void*,uint8_t,uint8_t,uint8_t);
void lxrp_transport(void*,double,double,bool);
void lxrp_process(void*,float*,uint32_t);
void lxrp_status(void*,float*);
bool lxrp_preview(void*);
size_t lxrp_json(void*,uint8_t*,size_t);
bool lxrp_load(void*,const uint8_t*,size_t);
void lxrp_meta(void*,const uint8_t*,size_t,uint32_t);
void lxrp_labels(void*,const uint8_t*,size_t);
void lxrp_info(uint32_t,char*,size_t,float*);
}
