// Rebuild native derivatives from the pinned, byte-verified source library.
// Run: node scripts/provisioning/build-native-library.mjs
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { compile } from './smarty-to-jinja.mjs';
import { adaptSource } from './adapt-source.mjs';

const root = new URL('../../', import.meta.url);
const manifest = JSON.parse(await readFile(new URL('docs/research/fusionpbx-provisioning-files.json', root), 'utf8'));
const inventory = JSON.parse(await readFile(new URL('docs/research/fusionpbx-all-vendors-inventory.json', root), 'utf8'));
const sourceRoot = new URL('crates/talkops-provisioning/reference/fusionpbx/', root);
const nativeRoot = new URL('crates/talkops-provisioning/templates/fusionpbx/', root);
const profiles = [], failures = [];
const check=process.argv.includes('--check');
async function writeGenerated(url,data){if(check){if(await readFile(url,'utf8')!==data)throw Error(`Generated file differs: ${url.pathname}`);}else{await mkdir(new URL('./',url),{recursive:true});await writeFile(url,data);}}
const knownYealink = new Set([...((await readFile(new URL('presets/phones/yealink.yaml', root), 'utf8')).matchAll(/^\s+- id: (\S+)/gm))].map(m => m[1]));
function family(vendor, model) {
  if(vendor==='grandstream'&&model==='wave')return 'app';
  if (['acrobits','groundwire','linphone','sipnetic','zoiper'].includes(vendor)) return 'app';
  if (vendor === '2n') return 'intercom';
  if (vendor === 'algo') return 'paging';
  if (/^(?:gxw|ta\d{4})/i.test(model)) return 'gateway';
  if (/^(?:ht\d|ta\d|obi\d{3}$|spa[23]\d\d\d)/i.test(model)) return 'ata';
  if(vendor==='fanvil'&&/^i\d/i.test(model)||vendor==='grandstream'&&/^gds/i.test(model))return 'intercom';
  if(vendor==='grandstream'&&/^wp\d/i.test(model)||vendor==='fanvil'&&/^w6\d/i.test(model)||vendor==='polycom'&&model==='8030')return 'wifi';
  if(vendor==='snom'&&/^m(?:3|9|\d{3})$/i.test(model))return 'dect';
  if (/^(?:dp|w[5678]\d|wp[0-9]|tgp|m[379]00)/i.test(model)) return 'dect';
  if (/^(?:cp|c[56][02]0|vcs|trio|gac|gvc)/i.test(model)||vendor==='polycom'&&/^[5678]000$/.test(model)) return 'conference';
  return 'desk';
}
for (const vendor of inventory.vendors) {
  const models = [];
  const sourceModels=[...vendor.template_directories];
  if(vendor.vendor==='grandstream'&&vendor.root_files.includes('gxv3380'))sourceModels.push('gxv3380');
  for (const model of sourceModels) {
    const prefix = `${vendor.vendor}/${model}/`;
    const files = manifest.files.filter(f => (f.path.startsWith(prefix)||f.path===`${vendor.vendor}/${model}`||vendor.vendor==='avaya'&&f.path==='avaya/J100Supgrade.txt') && !/\.(?:md|ott)$/i.test(f.path));
    const artifacts = []; let allSource = '';
    for (const file of files) {
      const bytes = await readFile(new URL(file.path, sourceRoot));
      const hash = createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex');
      if (hash !== file.blob) throw Error(`Source hash mismatch: ${file.path}`);
      let source = adaptSource(vendor.vendor,file.path,new TextDecoder('utf8', {fatal: true}).decode(bytes));
      allSource += source;
      try {
        const format=/\.xml$/i.test(file.path)||/^\s*(?:<\?xml[^>]*>\s*)?</.test(source)&&!source.startsWith('<<')?'xml':/\.json$/i.test(file.path)?'json':/\.csv$/i.test(file.path)?'csv':'text';
        const output = `{# MPL-1.1 derivative: FusionPBX ${manifest.revision}, ${file.path}, blob ${file.blob}. #}`+compile(source,{xml:format==='xml'});
        const target = new URL(`${file.path}.jinja`, nativeRoot);
        await writeGenerated(target, output);
        artifacts.push({ name: file.path.startsWith(prefix)?file.path.slice(prefix.length):file.path.split('/').at(-1), template: `${file.path}.jinja`, format:vendor.vendor==='mitel'&&file.path.endsWith('MN_Generic.cfg')||file.path.toLowerCase()==='snom/c520/{$mac}.xml'?'xml_fragment':file.path==='fanvil/h5/{$mac}.cfg'?'xml_fanvil':format });
      } catch (error) { failures.push({source:file.path, error:error.message}); }
    }
    let coalesced=0;
    if(vendor.vendor==='yealink'&&model==='t4x'){
      // Both source spellings resolve to the same MAC.boot filename. The
      // model-aware variant names the supplied CFG files. The older variant
      // refers to y000000000000.cfg, which is absent from this source profile.
      const modelBoot=artifacts.find(a=>a.name==='{mac}.boot');
      const genericBoot=artifacts.findIndex(a=>a.name==='{$mac}.boot');
      if(modelBoot&&genericBoot>=0){artifacts.splice(genericBoot,1);modelBoot.name='{$mac}.boot';coalesced++;}
    }
    let primary = artifacts.find(a => /\{\$(?:mac|address)\}/.test(a.name) && !/director|contact|phonebook|favorite|softkey/i.test(a.name) && /\.(?:cfg|xml|conf|cnf|ini)$/i.test(a.name))
      ?? artifacts.find(a => /^(?:config\.xml|template\.csv|qr_template\.txt|gxv3380)$/.test(a.name));
    if(vendor.vendor==='yealink'&&model==='cp965'){
      // CP965 refers to these standard Yealink directory files, but its source
      // folder omits them. Reuse the pinned T4x files in the same library.
      for(const name of ['directory.xml','favorite_setting.xml'])artifacts.push({name,template:`yealink/t4x/${name}.jinja`,format:'xml'});
    }
    if(vendor.vendor==='sangoma'){
      // The pinned Sangoma P4401/P4402/P4403 fields use the Htek directory
      // format, but omit the referenced file. Keep that source-format alias
      // explicit; device validation is still not_tested.
      artifacts.push({name:'directory.xml',template:'htek/uc903/phonebook.xml.jinja',format:'xml'});
    }
    if(vendor.vendor==='groundwire'){
      const template=`groundwire/${model}/directory.json.jinja`;
      await writeGenerated(new URL(template,nativeRoot),'{# Independently authored TalkOps AGPL-3.0-only web contacts export. #}{{ web_contacts | json }}');
      artifacts.push({name:'directory.json',template,format:'json'});
    }
    const fixed = [...allSource.matchAll(/\$(?:account|user)\.(\d+)\.(?:user_id|auth_id|password|user_password)\b/g)].map(m => Number(m[1]));
    fixed.push(...[...allSource.matchAll(/\$(?:user_id|auth_id|user_password|password)_(\d+)\b/g)].map(m=>Number(m[1])));
    const limits=[],loops=[];
    if(/\$row\s*=\s*\$lines(?:\.\$line|\[\$line\])/.test(allSource))limits.push(...[...allSource.matchAll(/\{for\s+\$line\s*=\s*1\s+to\s+(\d+)/g)].map(m=>Number(m[1])));
    for(const match of allSource.matchAll(/\{foreach\s+([^}]+)\}|\{\/foreach\}|\{if\s+\$row@index\s*(?:eq|==|>=)\s*(\d+)\s*\}\s*\{break\}/g)){
      if(match[1])loops.push(/^\$lines\s+as\s+\$row\b/.test(match[1]));
      else if(match[2]&&loops.at(-1))limits.push(Number(match[2])+1);
      else if(match[0]==='{/foreach}')loops.pop();
    }
    // The generic GXP16xx profile has two accounts. Its third account is
    // explicitly conditional on the GXP1630 selector in the original source.
    const accounts = vendor.vendor==='grandstream'&&model==='gxp16xx'?2:family(vendor.vendor,model)==='app'?1:Math.min(100,limits.length?Math.min(...limits):Math.max(1,...fixed));
    const id = `${vendor.vendor}-${model.replace(/[^a-zA-Z0-9_-]/g,'-').toLowerCase()}`;
    const settingPrefix=({poly:'polycom',linksys:'spa'})[vendor.vendor]??vendor.vendor;
    const settingNames=[...new Set([...allSource.matchAll(/\$([a-z][a-z0-9_]+)/g)].map(m=>m[1]))]
      .filter(name=>name.startsWith(settingPrefix+'_')||vendor.vendor==='acrobits'&&name==='code').sort();
    const settings=settingNames.map(key=>({key,label:key.replace(new RegExp('^'+settingPrefix+'_'),'').replaceAll('_',' '),
      kind:/password|passwd|passcode|(?:^|_)pin(?:_|$)|pincode|secret|private_key|token|^code$/.test(key)?'secret':new RegExp('\\{foreach\\s+\\$'+key+'\\s+as\\s+').test(allSource)?'list':'text',
      copyable:!/(?:password|passwd|passcode|(?:^|_)pin(?:_|$)|pincode|secret|private_key|token|hostname|host_name|static_ip|ip_address|mac_address|^code$)/.test(key)}));
    const key_groups=[...new Set([...allSource.matchAll(/\$keys(?:\[['"]([\w-]+)['"]\]|\.([\w-]+))/g)].map(m=>m[1]??m[2]))].sort();
    const state = allSource.trim() && primary && artifacts.length === files.length+(['groundwire','sangoma'].includes(vendor.vendor)?1:vendor.vendor==='yealink'&&model==='cp965'?2:0)-coalesced ? 'adapted' : 'blocked';
    profiles.push({ id, vendor:vendor.vendor, source_model:model, revision:manifest.revision, adaptation:state, device_validation:'not_tested',
      reason: state === 'blocked' ? (!allSource.trim()?'no_source_template':primary ? 'Source conversion requires review' : 'No device configuration artifact') : null,
      accounts, account_capacity_source:family(vendor.vendor,model)==='app'?'single_account_export':fixed.length||limits.length?'source_slots':'single_account_baseline', family:family(vendor.vendor,model), primary:primary?.name??null, artifacts, settings, key_groups,indexed_lines:/\$lines(?:\.\$line|\[\$line\])/.test(allSource) });
    if (state === 'adapted' && !(vendor.vendor==='yealink' && knownYealink.has(model))) models.push({id,name:model,family:family(vendor.vendor,model),accounts,line_keys:0});
    if(vendor.vendor==='grandstream'&&model==='gxp16xx'){
      profiles.push({...profiles.at(-1),id:'grandstream-gxp1630',accounts:3,template_selector:'grandstream/gxp1630'});
      models.push({id:'grandstream-gxp1630',name:'gxp1630',family:'desk',accounts:3,line_keys:0});
    }
  }
  if (models.length) {
    // JSON is valid YAML. Keep generated records stable and separate from manual profiles.
    await writeGenerated(new URL(`presets/phones/${vendor.vendor}-source.yaml`,root),JSON.stringify({vendor:vendor.vendor,status:'untested',models},null,2)+'\n');
  }
}
await writeGenerated(new URL('crates/talkops-provisioning/profiles/fusionpbx.json',root),JSON.stringify(profiles,null,2)+'\n');
await writeGenerated(new URL('docs/research/fusionpbx-conversion-report.json',root),JSON.stringify({revision:manifest.revision,profiles:profiles.length,failures},null,2)+'\n');
console.log(JSON.stringify({profiles:profiles.length,adapted:profiles.filter(p=>p.adaptation==='adapted').length,failures:failures.length,examples:failures.slice(0,12)},null,2));
