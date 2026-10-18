#pragma once
static constexpr int kDXMParams=292;
static constexpr int kDrumNotes[7]={36,45,47,38,49,42,46};
static const char* kOutputNames[8]={"Mix","Drum 1","Drum 2","Drum 3","Snare","Cymbal","Closed Hat","Open Hat"};
static double compareOutput(const float* left,const float* right,const float* expected,int frames,int bus){
  double peak=0.;
  for(int i=0;i<frames;i++)for(int c=0;c<2;c++){
    const float actual=c?right[i]:left[i],want=expected[i*16+bus*2+c];
    if(!std::isfinite(actual)||std::abs(actual-want)>0.000002f){
      std::fprintf(stderr,"bus %s sample %d/%d actual %.9g expected %.9g\n",kOutputNames[bus],i,c,actual,want);REQUIRE(false);
    }
    peak=std::max(peak,std::abs(double(actual)));
  }
  return peak;
}
