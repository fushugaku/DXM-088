import assert from 'node:assert/strict';
import {KnobDrag,wheelPixels,ParameterEdits} from '../web/knob-input.js';
const near=(a,b)=>assert.ok(Math.abs(a-b)<1e-10,`${a} != ${b}`);
const drag=new KnobDrag(.5,200);
near(drag.move(168,false),.6);
near(drag.move(168,true),.6); // Shift without motion must not undo the previous drag.
near(drag.move(136,true),.61);
near(drag.move(136,false),.61);
near(drag.move(104,false),.71);
drag.move(-2000,false);near(drag.move(-1999,false),1-1/320);
drag.move(2000,false);near(drag.move(1999,false),1/320);
near(wheelPixels({deltaY:2,deltaMode:0}),2);
near(wheelPixels({deltaY:3,deltaMode:1}),90);
assert.ok(wheelPixels({deltaY:2,deltaMode:0})/8000<.001); // A tiny trackpad motion must stay tiny.
const edits=new ParameterEdits();
edits.begin(0);edits.write(0,100,0);edits.write(0,110,10);
assert.equal(edits.accept(0,100,30),false); // Delayed previous host echo.
assert.equal(edits.accept(0,90,1000),false); // A held gesture owns its value even after the grace period.
assert.equal(edits.accept(0,110,1001),false); // Acknowledgement is not a new edit.
edits.end(0);assert.equal(edits.accept(0,120,1010),true); // Automation resumes after release.
edits.write(0,100,2000);assert.equal(edits.accept(0,90,2020),false);
assert.equal(edits.accept(0,90,2401),true); // An unacknowledged edit cannot suppress automation forever.
console.log('Knob input: Shift continuity, end-stop reversal, trackpad precision and delayed host echoes passed.');
