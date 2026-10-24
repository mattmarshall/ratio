// Exclusion, not create:true, is the OPFS claim. Never steal a live lease.
export function resourceFor(namespace) {
  if(!/^[A-Za-z0-9_-]{1,80}$/.test(namespace)) throw new Error('invalid OPFS backend namespace');
  return `ratio:opfs:v1:${namespace}`;
}
export async function heldCount(resource) {
  const state=await navigator.locks.query();return state.held.filter(lock=>lock.name===resource).length;
}
export async function ownNamespace(namespace,onOwner,onClient) {
  const resource=resourceFor(namespace);
  if(typeof DedicatedWorkerGlobalScope==='undefined' || !(self instanceof DedicatedWorkerGlobalScope)
      || !self.isSecureContext || !navigator.locks?.request || !navigator.locks?.query || !navigator.storage?.getDirectory)
    throw new Error('exclusive OPFS ownership requires a supported secure dedicated worker');
  await navigator.locks.request(resource,{mode:'exclusive',ifAvailable:true},async lock=>{
    if(!lock){await onClient(await heldCount(resource));return;}
    let access,alive=false;
    try {
      const root=await navigator.storage.getDirectory();
      const directory=await root.getDirectoryHandle('ratio-object-log-v1',{create:true});
      const file=await directory.getFileHandle(`${namespace}.objects`,{create:true});
      // Default readwrite access is exclusive. No readwrite-unsafe fallback.
      access=await file.createSyncAccessHandle();alive=true;
      await onOwner({resource,handle:access,alive:()=>alive,count:()=>heldCount(resource)});
    } finally {
      alive=false;if(access)access.close();
      // The file handle closes before the Web Lock callback returns.
    }
  });
}
