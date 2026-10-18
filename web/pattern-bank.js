export const PATTERN_COUNT=128;
export const BANK_SIZE=8;
export const BANK_COUNT=PATTERN_COUNT/BANK_SIZE;
export const patternLabel=p=>`${String.fromCharCode(65+Math.floor(p/BANK_SIZE))}${String(p%BANK_SIZE+1).padStart(2,'0')}`;
export const patternNote=(p,base=0)=>(p+base)%PATTERN_COUNT;
export const noteName=n=>['C','C♯','D','D♯','E','F','F♯','G','G♯','A','A♯','B'][n%12]+(Math.floor(n/12)-2);

export function upgradePatternBank(input){
 if(input?.version!==3||!Array.isArray(input.patterns)||input.patterns.length!==8||!Array.isArray(input.scenes)||input.scenes.length!==8)return input;
 const s=structuredClone(input);
 s.version=4;
 s.patterns=Array.from({length:PATTERN_COUNT},(_,p)=>structuredClone(input.patterns[p%BANK_SIZE]));
 s.scenes=Array.from({length:PATTERN_COUNT},(_,p)=>structuredClone(input.scenes[p%BANK_SIZE]));
 s.plugin={...s.plugin,version:2,baseNote:s.plugin?.baseNote??24};
 return s;
}
