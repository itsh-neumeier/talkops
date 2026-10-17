// Copy Git blob bytes, bypassing checkout newline conversion and file filters.
// Usage: node scripts/provisioning/import-reference.mjs /path/to/fusionpbx
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const root = new URL('../../', import.meta.url);
const manifest = JSON.parse(await readFile(new URL('docs/research/fusionpbx-provisioning-files.json', root),'utf8'));
const repository = process.argv[2];
if (!repository) throw Error('Specify the local FusionPBX Git repository');
const revision = execFileSync('git',['-C',repository,'rev-parse',manifest.revision],{encoding:'utf8'}).trim();
if (revision !== manifest.revision) throw Error('Source revision mismatch');
const bytes = execFileSync('git',['-C',repository,'cat-file','--batch'],{
  input:manifest.files.map(f=>f.blob).join('\n')+'\n',maxBuffer:128*1024*1024,
});
let offset=0;
for (const file of manifest.files) {
  const newline=bytes.indexOf(10,offset), header=bytes.subarray(offset,newline).toString('ascii');
  if(header!==`${file.blob} blob ${file.bytes}`) throw Error(`Unexpected blob header for ${file.path}`);
  const body=bytes.subarray(newline+1,newline+1+file.bytes);
  const hash=createHash('sha1').update(`blob ${body.length}\0`).update(body).digest('hex');
  if(hash!==file.blob)throw Error(`Hash mismatch for ${file.path}`);
  const target=new URL('crates/talkops-provisioning/reference/fusionpbx/'+file.path,root);
  await mkdir(new URL('./',target),{recursive:true});await writeFile(target,body);
  offset=newline+file.bytes+2;
}
if(offset!==bytes.length)throw Error('Unexpected trailing blob data');
console.log(`Imported ${manifest.files.length} original blobs from ${revision}.`);
