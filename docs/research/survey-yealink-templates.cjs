const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');
const assert = require('node:assert/strict');
// Research only. Read the pinned upstream checkout without executing its code.
// Usage: node survey-yealink-templates.cjs <fusionpbx-checkout> [output-directory]
assert(process.argv[2], 'Pass the path of the pinned FusionPBX checkout.');
const upstream = path.resolve(process.argv[2]);
const output = path.resolve(process.argv[3] || __dirname);
const prefix = 'resources/templates/provision/yealink/';
const revision = cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: upstream, encoding: 'utf8'}).trim();
assert.equal(revision, 'be0d0e3220e6b3666775fa46db6f1accc60c79c6', 'Use the reviewed upstream revision.');
const url = `https://github.com/fusionpbx/fusionpbx/blob/${revision}/`;
const tree = cp.execFileSync('git', ['ls-tree', '-rlz', 'HEAD', prefix], {cwd: upstream, encoding: 'utf8'});
const entries = tree.split('\0').filter(Boolean).map(row => {
  const [,mode,hash,bytes,file] = row.match(/^(\d+) blob ([a-f0-9]+)\s+(\d+)\t(.*)$/s);
  return {mode, blob: hash, bytes: Number(bytes), path: file};
});
const familyPatterns = {
  line: /^\s*linekey\./,
  programmable: /^\s*program(?:m)?ablekey\./,
  memory: /^\s*memorykey\./,
  expansion: /^\s*expansion_module\./,
  softkey_layout: /^\s*(?:softkey\.|custom_softkey_|phone_setting\.custom_softkey_enable)/,
  handset_keys: /^\s*(?:custom\.)?handset\.[^=]*defined_[^=]*key\./,
  dss_assets: /^\s*dsskey\./,
  alarm_button: /^\s*alarm\.button[.\s]/,
};
const fileRecords = entries.map(entry => {
  const text = fs.readFileSync(path.join(upstream, entry.path), 'utf8');
  const lines = text.split(/\r?\n/);
  const groups = {};
  for (const [group, re] of Object.entries(familyPatterns)) {
    const hits = [];
    lines.forEach((line, i) => {
      if (re.test(line) && line.includes('=')) {
        hits.push({line: i + 1, parameter: line.slice(0, line.indexOf('=')).trim()});
      }
    });
    if (hits.length) {
      const patterns = new Map();
      for (const hit of hits) {
        const pattern = hit.parameter.replace(/\.\d+(?=\.)/g, '.{slot}');
        if (!patterns.has(pattern)) patterns.set(pattern, hit.line);
      }
      groups[group] = {first_line: hits[0].line, assignment_count: hits.length,
        parameters: [...patterns].map(([pattern, first_line])=>({pattern, first_line}))};
    }
  }
  const loops = [...text.matchAll(/\$keys\[['"]([^'"]+)['"]\]/g)].map(m => m[1]);
  const comments = [];
  lines.forEach((line, i) => {
    if (/^\s*#/.test(line) && (
      /(?:programablekey|programmablekey|linekey|memorykey|expansion_module|softkey|dsskey)/i.test(line) && /(?:ranges?|[0-9]|type|key)/i.test(line)
      || /^\s*#.*\b\d+\s*-\s*(?:N\/A|Forward|XML|DND|BLF|Line|Speed|Spead|Redial)/i.test(line)
    )) comments.push({line: i + 1, text: line.trim()});
  });
  const softkeyStates = [...new Set([...text.matchAll(/^\s*custom_softkey_([a-z_]+)\.url\s*=/gm)].map(m => m[1]))].sort();
  const stateFlags = [...new Set([...text.matchAll(/^\s*softkey\.[^.]+\.use\.([a-z_]+)\s*=/gm)].map(m => m[1]))].sort();
  const fixedTypeSlots = {};
  for (const [group, prefixName] of [['line', 'linekey'], ['programmable', 'programablekey'], ['memory', 'memorykey']]) {
    fixedTypeSlots[group] = [...new Set([...text.matchAll(new RegExp(`^\\s*${prefixName}\\.(\\d+)\\.type\\s*=`, 'gm'))].map(m => Number(m[1])))].sort((a,b)=>a-b);
  }
  const typeCodeHints = [...new Set(comments.filter(c=>/^#(?:0-N\/A|22-XML|33-Status)/i.test(c.text)).flatMap(c=>[...c.text.matchAll(/(\d+)\s*-\s*[A-Za-z]/g)].map(m=>Number(m[1]))))].sort((a,b)=>a-b);
  return {...entry, model: entry.path.slice(prefix.length).split('/')[0], loops: [...new Set(loops)].sort(), groups, comment_lines: comments.map(c=>c.line), typeCodeHints, softkeyStates, stateFlags, fixedTypeSlots};
});
const modelIds = [...new Set(fileRecords.map(file => file.model))].sort();
const models = modelIds.map(id => {
  const files = fileRecords.filter(f => f.model === id);
  return {
    id,
    files: files.map(f => f.path),
    groups: Object.keys(familyPatterns).filter(group => files.some(f => f.groups[group])),
    loops: [...new Set(files.flatMap(f => f.loops))].sort(),
    softkeyStates: [...new Set(files.flatMap(f => f.softkeyStates))].sort(),
    stateFlags: [...new Set(files.flatMap(f => f.stateFlags))].sort(),
    typeCodeHints: [...new Set(files.flatMap(f => f.typeCodeHints))].sort((a,b)=>a-b),
    fixedTypeSlots: Object.fromEntries(['line', 'programmable', 'memory'].map(group => [group, [...new Set(files.flatMap(f => f.fixedTypeSlots[group]))].sort((a,b)=>a-b)])),
  };
});
const config = fs.readFileSync(path.join(upstream, 'app/devices/app_config.php'), 'utf8');
const start = config.indexOf('//yealink details');
const end = config.indexOf('//snom details', start);
assert(start >= 0 && end > start);
const functions = config.slice(start, end).split('$z++;').flatMap(block => {
  const type = block.match(/\['type'\]\s*=\s*"([^"]+)"/);
  const value = block.match(/\['value'\]\s*=\s*"([^"]+)"/);
  return type && value ? [{type: type[1], code: Number(value[1])}] : [];
});
assert.equal(entries.length, 433);
assert.equal(models.length, 70);
assert.equal(functions.find(f => f.type === 'switch_account_up').code, 48);
assert(models.find(m => m.id === 't41p').groups.includes('programmable'));
assert(models.find(m => m.id === 't41p').softkeyStates.includes('talking'));
const tallies = Object.fromEntries(Object.keys(familyPatterns).map(group => [group, models.filter(m=>m.groups.includes(group)).length]));
const result = {revision, source: 'https://github.com/fusionpbx/fusionpbx', templateRoot: prefix, fileCount: entries.length, modelCount: models.length, countsByGroup: tallies, functions, models, files: fileRecords};
fs.mkdirSync(output, {recursive: true});
fs.writeFileSync(path.join(output, 'yealink-template-inventory.json'), '{\n'+Object.entries(result).map(([key,value])=>
  `  ${JSON.stringify(key)}: `+(Array.isArray(value)?'[\n'+value.map(item=>'    '+JSON.stringify(item)).join(',\n')+'\n  ]':JSON.stringify(value))
).join(',\n')+'\n}\n');
const lines = [
  '# FusionPBX Yealink key template inventory', '',
  'Status: Research for review. No application change is approved.', '',
  `Source revision: [${revision}](${url.replace('/blob/', '/commit/').replace(/\/$/, '')}).`, '',
  `Inspected all ${entries.length} files in all ${models.length} Yealink directories.`,
  'These directories include model families, conference phones and DECT bases.',
  'Directory names are not a count of tested phone models.', '',
  'The scan reads template assignments, key loops and key comments.',
  'It does not execute PHP or Smarty. It does not render a phone configuration.',
  'The JSON companion records every file, upstream Git blob and key parameter pattern.',
  'It records the first source line for each pattern. Numeric slots become `{slot}`.', '',
  'To reproduce, check out the pinned FusionPBX revision with the Yealink tree',
  'and `app/devices/app_config.php`. Then run:', '',
  '```sh', 'node docs/research/survey-yealink-templates.cjs /path/to/fusionpbx', '```', '',
  '## Evidence rules', '',
  '- A table entry means that the template contains an active parameter assignment.',
  '- An empty assignment is evidence of a template field, not an active phone feature.',
  '- A loop can accept key IDs that the model does not have. A loop proves no key count.',
  '- Literal slots and comment ranges can be copied from a different model.',
  '- Do not use this table alone to enable a hardware capability in TalkOps.',
  '- The function registry is vendor-wide. It is not a per-model allowed-function list.',
  '- Comments and registry entries can disagree. Keep their evidence separate.', '',
  '## Groups by directory', '',
  'L = `linekey`; P = `programablekey`; M = `memorykey`; E = `expansion_module`.',
  'S = custom softkey layout; H = defined handset keys; D = DSS icon assets;',
  'A = alarm button parameters. A dash means no assignment was found.', '',
  '| Directory | Files | L | P | M | E | S | H | D | A | FusionPBX key loops |',
  '| --- | ---: | --- | --- | --- | --- | --- | --- | --- | --- | --- |',
];
for (const m of models) {
  lines.push(`| [${m.id}](https://github.com/fusionpbx/fusionpbx/tree/${revision}/${prefix}${m.id}) | ${m.files.length} | ${Object.keys(familyPatterns).map(g=>m.groups.includes(g)?'yes':'-').join(' | ')} | ${m.loops.map(x=>'`'+x+'`').join(', ')||'-'} |`);
}
lines.push('', '## Vendor function registry', '',
  `Source: [app/devices/app_config.php](${url}app/devices/app_config.php#L1290).`, '',
  `The Yealink registry contains ${functions.length} entries. These entries need a model and key-group filter.`, '',
  '| Code | FusionPBX function ID |', '| ---: | --- |');
for (const f of functions) lines.push(`| ${f.code} | \`${f.type}\` |`);
lines.push('', '## Per-directory details', '',
  'Each section links the configuration files that contain key assignments.',
  'The slot lists below are literal assignments in the source. They are not model capacities.',
  'The JSON record links comment lines without copying the source comment text.',
  'Type-code hints come from selected legacy type-list comments. An empty list is not a restriction.', '');
for (const m of models) {
  lines.push(`### ${m.id}`, '',
    `- Parameter groups: ${m.groups.join(', ') || 'None found'}.`,
    `- Key loops: ${m.loops.join(', ') || 'None found'}.`,
    `- Softkey URL states: ${m.softkeyStates.join(', ') || 'None found'}.`,
    `- Inline softkey state flags: ${m.stateFlags.join(', ') || 'None found'}.`);
  lines.push(`- Legacy type-code comment hints: ${m.typeCodeHints.join(', ') || 'No parsed type list; check source documentation'}.`);
  for (const [g, slots] of Object.entries(m.fixedTypeSlots)) if (slots.length) lines.push(`- Literal ${g} type slots: ${slots.join(', ')}.`);
  for (const file of fileRecords.filter(f=>f.model===m.id && Object.keys(f.groups).length)) {
    const firstLine = Math.min(...Object.values(file.groups).map(group=>group.first_line));
    const relative = file.path.slice(prefix.length+m.id.length+1);
    lines.push(`- [${relative}](${url}${encodeURI(file.path).replace(/\$/g,'%24')}#L${firstLine}): ${Object.keys(file.groups).join(', ')}.`);
  }
  lines.push('');
}
fs.writeFileSync(path.join(output, 'yealink-template-inventory.md'), lines.join('\n'));
console.log(JSON.stringify({revision, files: entries.length, directories: models.length, functions: functions.length, groups:tallies, t41p: models.find(m=>m.id==='t41p')}, null, 2));
