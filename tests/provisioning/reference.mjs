import { readFile,readdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const root=new URL('../../',import.meta.url);
const manifest=JSON.parse(await readFile(new URL('docs/research/yealink-template-inventory.json',root),'utf8'));
const base=new URL('crates/talkops-provisioning/reference/fusionpbx/yealink/',root);
for(const file of manifest.files){
  const bytes=await readFile(new URL(file.path.split('/yealink/')[1],base));
  assert.equal(bytes.length,file.bytes,file.path);
  const hash=createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex');
  assert.equal(hash,file.blob,file.path);
}
const all=await readdir(base,{recursive:true,withFileTypes:true});
assert.equal(all.filter(e=>e.isFile()).length,433);
console.log(`Verified ${manifest.files.length} original FusionPBX Git blobs at ${manifest.revision}.`);
const complete=JSON.parse(await readFile(new URL('docs/research/fusionpbx-provisioning-files.json',root),'utf8'));
const completeBase=new URL('crates/talkops-provisioning/reference/fusionpbx/',root);
for(const file of complete.files){
  const bytes=await readFile(new URL(file.path,completeBase));
  assert.equal(bytes.length,file.bytes,file.path);
  assert.equal(createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex'),file.blob,file.path);
}
const completeFiles=await readdir(completeBase,{recursive:true,withFileTypes:true});
assert.equal(completeFiles.filter(e=>e.isFile()).length,complete.files.length+2); // license files
console.log(`Verified all ${complete.files.length} original provisioning blobs at ${complete.revision}.`);
const protocol=JSON.parse(await readFile(new URL('docs/research/fusionpbx-function-sources.json',root),'utf8'));
for(const file of protocol.files){
 const bytes=await readFile(new URL(`crates/talkops-provisioning/reference/fusionpbx-apps/${file.path}`,root));
 assert.equal(bytes.length,file.bytes,file.path);
 assert.equal(createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex'),file.blob,file.path);
}
console.log(`Verified ${protocol.files.length} original protocol/function blobs at ${protocol.revision}.`);
