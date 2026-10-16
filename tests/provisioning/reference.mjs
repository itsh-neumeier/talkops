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
