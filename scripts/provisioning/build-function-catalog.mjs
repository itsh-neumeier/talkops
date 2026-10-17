// Read only literal vendor function records. This is not a PHP interpreter.
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const root=new URL('../../',import.meta.url);
const manifest=JSON.parse(await readFile(new URL('docs/research/fusionpbx-function-sources.json',root),'utf8'));
for(const entry of manifest.files){
    const bytes=await readFile(new URL(`crates/talkops-provisioning/reference/fusionpbx-apps/${entry.path}`,root));
    if(createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex')!==entry.blob)throw Error(`Source hash mismatch: ${entry.path}`);
}
const source=await readFile(new URL('crates/talkops-provisioning/reference/fusionpbx-apps/app/devices/app_config.php',root),'utf8');
const vendors={};let vendor=null,record=null;
function finish(){if(vendor&&record?.uuid&&record.type!==undefined&&record.value!==undefined)(vendors[vendor]??=[]).push(record);record=null;}
const statements=/\$vendors\[\$y\]\['name'\]\s*=\s*"([^"\r\n]+)"|\$vendors\[\$y\]\['functions'\]\[\$z\]\['(uuid|type|subtype|value|description)'\]\s*=\s*"((?:\\.|[^"\\])*)"\s*;/g;
for(const match of source.matchAll(statements)){
    if(match[1]){finish();vendor=match[1];continue;}
    if(match[2]==='uuid'){finish();record={};}
    if(record)record[match[2]]=match[3].replace(/\\(["\\])/g,'$1');
}
finish();
if(vendors['escene programmable']){
    vendors.escene=vendors.escene.map(record=>({...record,groups:['line','memory','expansion']}));
    vendors.escene.push(...vendors['escene programmable'].map(record=>({...record,groups:['programmable']})));
    delete vendors['escene programmable'];
}
const output=JSON.stringify({revision:manifest.revision,vendors},null,2)+'\n';
const target=new URL('crates/talkops-provisioning/profiles/functions.json',root);
if(process.argv.includes('--check')){if(await readFile(target,'utf8')!==output)throw Error('Function catalog differs');}else{await writeFile(target,output);}
console.log(JSON.stringify(Object.fromEntries(Object.entries(vendors).map(([vendor,rows])=>[vendor,rows.length]))));
