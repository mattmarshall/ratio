// This fs-backed synchronous-handle shim is CI simulation, not live OPFS or
// proof of browser ownership. The exact Rust adapter/module is exercised.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import assert from 'node:assert/strict';
import {ObjectLog} from './object-log.mjs';
import {imports,validate,probe} from './host.mjs';
import {encodeRequest,decodeReply,text} from './wire.mjs';
const [modulePath,baselinePath]=process.argv.slice(2);
const expected=JSON.parse(fs.readFileSync(baselinePath,'utf8'));
const moduleObject=new WebAssembly.Module(fs.readFileSync(modulePath));
const root=fs.mkdtempSync(path.join(process.env.TEST_TMPDIR||os.tmpdir(),'opfs-'));
class Handle {
  constructor(file){this.fd=fs.openSync(file,fs.constants.O_RDWR|fs.constants.O_CREAT,0o600);this.closed=false;this.interrupt=false;this.failAfterFlush=false;}
  getSize(){return fs.fstatSync(this.fd).size;}
  read(bytes,{at}){return fs.readSync(this.fd,bytes,0,bytes.length,at);}
  write(bytes,{at}){
    if(this.interrupt){this.interrupt=false;const n=Math.max(1,Math.floor(bytes.length/2));
      fs.writeSync(this.fd,bytes,0,n,at);fs.fsyncSync(this.fd);throw new Error('injected partial append');}
    return fs.writeSync(this.fd,bytes,0,bytes.length,at);
  }
  truncate(size){fs.ftruncateSync(this.fd,size);}
  flush(){fs.fsyncSync(this.fd);if(this.failAfterFlush){this.failAfterFlush=false;throw new Error('injected lost acknowledgment after flush');}}
  close(){if(!this.closed){fs.closeSync(this.fd);this.closed=true;}}
}
function context(){
  const bindings=new Map(),handles=[];let instance,trapNextValidation=false;
  instance=new WebAssembly.Instance(moduleObject,{env:imports(()=>instance.exports.memory,bindings)});
  return {
    bindings,
    trapNext(){trapNextValidation=true;},
    open(id,file){const handle=new Handle(file);handles.push(handle);
      const log=new ObjectLog(handle,frame=>{if(trapNextValidation){trapNextValidation=false;instance.exports.ratio_opfs_test_trap();}return validate(instance,frame);});
      bindings.set(id,{log,active:()=>!handle.closed});return {handle,log};},
    request(operation,binding=1){const result=decodeReply(probe(instance,encodeRequest({correlationId:`${binding}-${operation}`,operation,binding})));
      return result;},
    close(){for(const handle of handles)handle.close();},
  };
}
const first=path.join(root,'first.objects'),second=path.join(root,'second.objects'),partial=path.join(root,'partial.objects');
let c=context();c.open(1,first);c.open(2,second);
for(const binding of [1,2]){const reply=c.request(2,binding);assert.equal(reply.passed,true,reply.detail);assert.deepEqual(JSON.parse(text(reply.fixtureJson)),expected);}
const appended=c.request(4,1);assert.equal(appended.passed,true,appended.detail);assert.match(appended.detail,/drill height 2/);
assert.deepEqual(JSON.parse(text(c.request(3,2).fixtureJson)),expected,'a second binding changed');
assert.equal(c.request(3,0).passed,false,'zero binding fell back to another backend');
c.bindings.delete(1);const missing=c.request(3,1);assert.equal(missing.passed,false);assert.match(missing.detail,/exclusive owner/);
c.close();
c=context();c.open(1,first);c.open(2,second);
const cold=c.request(3,1);assert.equal(cold.passed,true,cold.detail);assert.match(cold.detail,/drill height 2/);assert.deepEqual(JSON.parse(text(cold.fixtureJson)),expected);
assert.deepEqual(JSON.parse(text(c.request(3,2).fixtureJson)),expected);
const injected=c.open(3,partial);assert.equal(c.request(2,3).passed,true);
const acknowledgedEnd=injected.log.end;injected.handle.interrupt=true;
const interrupted=c.request(6,3);assert.equal(interrupted.passed,true,interrupted.detail);assert.equal(interrupted.writeAcknowledged,false);
assert.equal(injected.log.poisoned,true);assert.equal(c.request(3,3).passed,false,'poisoned owner served figures');
assert.ok(fs.statSync(partial).size>acknowledgedEnd);c.close();
c=context();const recovered=c.open(3,partial);assert.ok(recovered.log.recoveredTailBytes>0);
assert.equal(fs.statSync(partial).size,acknowledgedEnd);assert.deepEqual(JSON.parse(text(c.request(3,3).fixtureJson)),expected);
assert.equal(c.request(4,3).passed,true);c.close();
const unknown=path.join(root,'unknown.objects');c=context();const uncertain=c.open(4,unknown);
assert.equal(c.request(2,4).passed,true);uncertain.handle.failAfterFlush=true;
const noReply=c.request(6,4);assert.equal(noReply.passed,true,noReply.detail);assert.equal(noReply.writeAcknowledged,false);c.close();
c=context();const complete=c.open(4,unknown);assert.equal(complete.log.recoveredTailBytes,0);
const reconciled=c.request(3,4);assert.equal(reconciled.passed,true,reconciled.detail);assert.match(reconciled.detail,/drill height 2/);
const beforeRetry=fs.statSync(unknown).size;const retry=c.request(4,4);assert.equal(retry.passed,false);assert.match(retry.detail,/reconciliation/);
assert.equal(fs.statSync(unknown).size,beforeRetry,'unknown-outcome retry added another posting');c.close();
const trapped=path.join(root,'trapped.objects');c=context();c.open(5,trapped);assert.equal(c.request(2,5).passed,true);
const beforeTrap=fs.statSync(trapped).size;c.trapNext();assert.throws(()=>c.request(4,5),WebAssembly.RuntimeError);
assert.equal(fs.statSync(trapped).size,beforeTrap,'nested trap wrote a frame');c.close();
c=context();c.open(5,trapped);const afterTrap=c.request(3,5);assert.equal(afterTrap.passed,true,afterTrap.detail);assert.match(afterTrap.detail,/drill height 1/);c.close();
const corrupt=path.join(root,'corrupt.objects');fs.copyFileSync(partial,corrupt);
const bytes=fs.readFileSync(corrupt);bytes[6]^=1;fs.writeFileSync(corrupt,bytes);
c=context();assert.throws(()=>c.open(1,corrupt),/corrupt/);c.close();
const duplicate=path.join(root,'duplicate.objects');fs.copyFileSync(partial,duplicate);
const good=fs.readFileSync(duplicate),length=good.readUInt32LE(0);fs.appendFileSync(duplicate,good.subarray(0,length+8));
c=context();assert.throws(()=>c.open(1,duplicate),/duplicate/);c.close();
console.log('SIMULATED handle: actual Rust wasm adapter matches all native fixture bytes/pins/figures, isolated bindings, binary/UTF-8/u64 boundaries, claims/holes, cold recovery, partial append refusal, corruption and duplicate-frame refusal. Browser ownership is separate evidence.');
