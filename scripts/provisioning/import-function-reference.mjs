// Import protocol reference data as original Git blobs. Never execute PHP.
import {execFileSync} from 'node:child_process';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const root=new URL('../../',import.meta.url);
const {revision}=JSON.parse(await readFile(new URL('docs/research/fusionpbx-provisioning-files.json',root),'utf8'));
const repository=process.argv[2];if(!repository)throw Error('Specify the local FusionPBX repository');
const files=[];
for(const path of ['app/devices/app_config.php','app/provision/resources/classes/provision.php']){
    const blob=execFileSync('git',['-C',repository,'rev-parse',`${revision}:${path}`],{encoding:'utf8'}).trim();
    const bytes=execFileSync('git',['-C',repository,'cat-file','blob',blob],{maxBuffer:8*1024*1024});
    if(createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex')!==blob)throw Error('Source hash mismatch');
    const target=new URL(`crates/talkops-provisioning/reference/fusionpbx-apps/${path}`,root);
    await mkdir(new URL('./',target),{recursive:true});await writeFile(target,bytes);
    files.push({path,blob,bytes:bytes.length});
}
await writeFile(new URL('docs/research/fusionpbx-function-sources.json',root),JSON.stringify({revision,files},null,2)+'\n');
console.log(`Imported ${files.length} protocol reference blobs at ${revision}.`);
