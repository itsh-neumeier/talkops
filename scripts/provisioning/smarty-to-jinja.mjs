// Build-time source conversion only. TalkOps never evaluates Smarty or PHP.
// Derived templates retain the source notices and MPL-1.1 provenance.
function bracketEnd(text,start){let depth=1,quote='',i=start+1;for(;i<text.length&&depth;i++){const ch=text[i];if(quote){if(ch===quote&&text[i-1]!=='\\')quote='';}else if('"\''.includes(ch))quote=ch;else if(ch==='[')depth++;else if(ch===']')depth--;}if(depth)throw Error('Unclosed index');return i;}

function referenceEnd(text,start){
  let i=start+(text[start]==='$'?1:0);
  const root=text.slice(i).match(/^\w+/);if(!root)return start+1;i+=root[0].length;
  while(i<text.length){
    if(text[i]==='.'||text[i]==='@'){const m=text.slice(i+1).match(/^\w+/);if(!m)break;i+=m[0].length+1;}
    else if(text[i]==='[')i=bracketEnd(text,i);
    else break;
  }return i;
}
function replaceVariables(text,replace){let result='',cursor=0;while(cursor<text.length){const start=text.indexOf('$',cursor);if(start<0)break;const end=referenceEnd(text,start);result+=text.slice(cursor,start)+replace(text.slice(start,end),start,end);cursor=end;}return result+text.slice(cursor);}

function split(text, delimiter) {
  let depth = 0, quote = '', start = 0, result = [];
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (quote) { if (ch === quote && text[i - 1] !== '\\') quote = ''; }
    else if ('"\''.includes(ch)) quote = ch;
    else if ('([{'.includes(ch)) depth++;
    else if (')]}'.includes(ch)) depth--;
    else if (!depth && text.startsWith(delimiter, i)) {
      result.push(text.slice(start, i)); start = i + delimiter.length; i += delimiter.length - 1;
    }
  }
  return [...result, text.slice(start)];
}

function call(text, name, replace) {
  const pattern = new RegExp(`\\b${name}\\s*\\(`, 'g');
  let match;
  while ((match = pattern.exec(text))) {
    let start = pattern.lastIndex, depth = 1, quote = '', end = start;
    for (; end < text.length; end++) {
      const ch = text[end];
      if (quote) { if (ch === quote && text[end - 1] !== '\\') quote = ''; }
      else if ('"\''.includes(ch)) quote = ch;
      else if (ch === '(') depth++;
      else if (ch === ')' && --depth === 0) break;
    }
    if (depth) throw Error(`Unclosed ${name} call`);
    text = text.slice(0, match.index) + replace(split(text.slice(start, end), ',')) + text.slice(end + 1);
    pattern.lastIndex = 0;
  }
  return text;
}

function attributes(text) {
  let result = {}, cursor = text.indexOf(' ') + 1;
  while(cursor < text.length) {
    const match=text.slice(cursor).match(/^\s*(\w+)\s*=\s*/);
    if(!match)break;
    const name=match[1],start=cursor+match[0].length;
    let end=start,depth=0,quote='';
    for(;end<text.length;end++) {
      const ch=text[end];
      if(quote){if(ch===quote&&text[end-1]!=='\\')quote='';}
      else if('"\''.includes(ch))quote=ch;
      else if('([{'.includes(ch))depth++;
      else if(')]}'.includes(ch))depth--;
      else if(!depth&&/^\s+\w+\s*=/.test(text.slice(end)))break;
    }
    result[name]=text.slice(start,end).trim();cursor=end;
  }
  return result;
}
function unquote(text) { return /^['"]/.test(text) ? text.slice(1, -1) : text; }

export function compile(source, options={}) {
  source=source.replace(/\{\s+(?=(?:if|foreach|assign|for|elseif)\b)/g,'{');
  if(options.xml)source=source.replace(/<--(?=\s)/g,'<!--')
    .replace(/<!--\s*(\{(?:if|elseif|else|\/if)[^}]*\})\s*-->/g,'$1')
    .replace(/<!--([\s\S]*?)-->/g,(_,body)=>'<!--'+body.replace(/--/g,'- -').replace(/-$/,'- ')+'-->');
  const scopes=[];
  const local=name=>scopes.some(scope=>scope.includes(name));
  // Upstream T20P ends with an unterminated variable. Reference bytes stay unchanged.
  source=source.replace(/\{\$yealink_sip_call_fail_use_reason_enable(?=\r?\n$)/,'{$yealink_sip_call_fail_use_reason_enable}');
  const assigned = new Set([...source.matchAll(/\{(?:assign|append)\s+var\s*=\s*["']?(\w+)/g)].map(m => m[1]));
  for (const m of source.matchAll(/\{\$(\w+)(?:(?:\.[\w]+)|(?:\[[^\]]+\]))*\s*=(?!=)/g)) assigned.add(m[1]);
  function expr(text) {
    text = text.trim();
    text=text.replace(/(\$[\w.\[\]$]+\.enabled)\s*(==|eq|!=|ne)\s*(['"])(true|false)\3/g,'$1 $2 $4');
    if(text.startsWith('[')&&text.endsWith(']')) {
      const items=split(text.slice(1,-1),',').filter(s=>s.trim());
      if(items.some(s=>split(s,'=>').length>1))return '{'+items.map(s=>split(s,'=>').map(expr).join(':')).join(',')+'}';
      return '['+items.map(expr).join(',')+']';
    }
    const ternary=split(text,'?');
    if(ternary.length===2){const branches=split(ternary[1],':');if(branches.length===2)return `(${expr(branches[0])} if ${expr(ternary[0])} else ${expr(branches[1])})`;}
    if(text.startsWith('(')&&text.endsWith(')')){
      let depth=0,quote='',whole=true;
      for(let i=0;i<text.length-1;i++){const ch=text[i];if(quote){if(ch===quote&&text[i-1]!=='\\')quote='';}else if('"\''.includes(ch))quote=ch;else if(ch==='(')depth++;else if(ch===')'&&!--depth){whole=false;break;}}
      if(whole)return '('+expr(text.slice(1,-1))+')';
    }
    text=text.replace(/\.\{(\$[^{}]+)\}/g,'[$1]')
      .replace(/\$(\w+)\{(\$[^{}]+)\}/g,(_,prefix,value)=>`(lookup(variables | default(none), [${JSON.stringify(prefix)} ~ ${expr(value)}]) | default(''))`)
      .replace(/\{(\$[^{}]+)\}/g,'$1')
      .replace(/\.(\$\w+)/g, (_,name)=>`[${name}]`)
      .replace(/(\$[\w.\[\]$"']+)\|in_array:(\$[\w.\[\]$"']+)/g,'in_array($1,$2)');
    text=call(text,'array_values',args=>`(${expr(args[0])} | values)`);
    text=call(text,'ucfirst',args=>`(${expr(args[0])} | capitalize)`);
    const filters = split(text, '|').filter((part, i, all) => part || all[i-1] === '');
    // Logical || must be converted before a Smarty filter is parsed.
    if (text.includes('||')) return split(text, '||').map(expr).join(' or ');
    if (filters.length > 1) {
      let output = expr(filters.shift());
      for (const filter of filters) {
        const native=filter.trim().match(/^(default|replace|upper|lower|escape|md5|int|length|trim|values)\((.*)\)$/);
        if(native){output=`(${output} | ${native[1]}(${native[2]}))`;continue;}
        const [name, ...args] = split(filter, ':').map(s => s.trim());
        if (!['default','replace','upper','lower','escape','md5','cat','intval','string_format','count','trim','truncate','capitalize','values','int','length'].includes(name)) throw Error(`Unsupported filter ${name}`);
        if (name === 'escape') output = `(${output} | escape)`;
        else if (name === 'cat') output = `(${output} ~ ${expr(args[0])})`;
        else if (name === 'intval') output = `(${output} | int)`;
        else if (name === 'string_format') output = `(${output} | int)`;
        else if (name === 'count') output = `(${output} | length)`;
        else output = `(${output} | ${name}${args.length ? '('+args.map(expr).join(',')+')' : ''})`;
      }
      return output;
    }
    text = call(text, 'isset', args => '('+args.map(arg => {
      const value = arg.trim().startsWith('$')?ref(arg.trim()):expr(arg); return `(${value} is defined and ${value} is not none)`;
    }).join(' and ')+')');
    text = call(text, 'empty', args => `(not ${expr(args[0])})`);
    text = call(text, 'in_array', args => `(${expr(args[0])} in ${expr(args[1])})`);
    text = call(text, 'array', args => `[${args.map(expr).join(',')}]`);
    text = call(text, 'count', args => `(${expr(args[0])} | length)`);
    text = call(text, 'is_array', args => `(${expr(args[0])} is iterable)`);
    text = call(text, 'substr', args => `substring(${args.map(expr).join(',')})`);
    text = call(text, 'filter_var',args=>{
      if(args[1]?.trim()==='FILTER_VALIDATE_BOOLEAN')return `php_bool(${expr(args[0])})`;
      if(args[1]?.trim()==='FILTER_VALIDATE_IP')return `ip_address(${expr(args[0])})`;
      throw Error('Unsupported filter_var validation');
    });
    // Protect string literals from identifier/operator conversion.
    const strings = [];
    text = text.replace(/"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'/g, value => {
      const index = strings.push(value) - 1; return `__STR${index}__`;
    });
    text = replaceVariables(text, (value,start,end) => {
      const numeric=/^\s*(?:[-+*/%]|(?:==|!=|>=|<=|>|<|eq\b|ne\b|ge\b|le\b)\s*-?\d)/.test(text.slice(end))||/(?:[-+*/%]|\d\s*(?:==|!=|>=|<=|>|<|eq|ne|ge|le))\s*$/.test(text.slice(0,start));
      return `(${ref(value)} | default(${numeric?'0':"''"})${numeric?' | int':''})`;
    })
      .replace(/&&/g, ' and ').replace(/\beq\b/g, '==').replace(/\bne\b/g, '!=')
      .replace(/\bneq\b/g,'!=')
      .replace(/\bgte\b|\bge\b/g, '>=').replace(/\blte\b|\ble\b/g, '<=')
      .replace(/\bgt\b/g, '>').replace(/\blt\b/g, '<').replace(/\bmod\b/g, '%')
      .replace(/!(?!=)/g, ' not ').replace(/===/g, '==').replace(/!==/g, '!=');
    text=text.replace(/(?<![=!<>])=(?![=>])/g,'==');
    if (text.includes('=>')) text = text.replace(/^\[/, '{').replace(/\]$/, '}').replace(/=>/g, ':');
    return text.replace(/__STR(\d+)__/g, (match, index) => strings[Number(index)]??match);
  }
  function ref(text) {
    text=text.replace(/\.\{(\$[^{}]+)\}/g,'[$1]').replace(/\{(\$[^{}]+)\}/g,'$1').replace(/\.(\$\w+)/g,(_,name)=>`[${name}]`);
    if(/^\$smarty\.foreach\./.test(text)) return text.endsWith('.iteration')?'loop.index':text.endsWith('.index')?'loop.index0':text.endsWith('.first')?'loop.first':'loop.last';
    text = text.replace(/^\$/, '').replace(/@(index|iteration|first|last)/g, (_, name) => `__LOOP_${name}__`);
    if (/__LOOP_/.test(text)) return ({index:'loop.index0',iteration:'loop.index',first:'loop.first',last:'loop.last'})[text.match(/__LOOP_(\w+)__/)[1]];
    const base = text.match(/^\w+/)?.[0];
    const path=[];let cursor=base.length;
    while(cursor<text.length){
      if(text[cursor]==='.'){const part=text.slice(cursor+1).match(/^\w+/);if(!part)throw Error(`Invalid reference ${text}`);path.push(JSON.stringify(part[0]));cursor+=part[0].length+1;}
      else if(text[cursor]==='['){const end=bracketEnd(text,cursor);const part=text.slice(cursor+1,end-1);path.push(/^\w+$/.test(part)&&!/^__STR\d+__$/.test(part)?JSON.stringify(part):expr(part));cursor=end;}
      else throw Error(`Invalid reference suffix ${text.slice(cursor)}`);
    }
    if(assigned.has(base)&&!local(base))path.unshift(JSON.stringify(base));
    const root=assigned.has(base)&&!local(base)?'ns':base;
    return path.length?`lookup(${root} | default(none), [${path.join(',')}])`:root;
  }
  function assignment(name, value, index) {
    name = unquote(name);
    if(/^\$\w+\+\+$/.test(value.trim()))return assignment(name,value.trim().slice(0,-2),index)+assignment(value.trim().slice(1,-2),value.trim().slice(0,-2)+'+1');
    if(/[.\[]/.test(name)) {
      const base=name.match(/^\w+/)[0],parts=[];let cursor=base.length;
      while(cursor<name.length){if(name[cursor]==='.'){const part=name.slice(cursor+1).match(/^\w+/)[0];parts.push(JSON.stringify(part));cursor+=part.length+1;}else{const end=bracketEnd(name,cursor);parts.push(expr(name.slice(cursor+1,end-1)));cursor=end;}}
      const root=local(base)?base:'ns.'+base;
      function update(depth){const container=depth?`lookup(${root} | default({}), [${parts.slice(0,depth).join(',')}])`:root;return `put(${container} | default({}), ${parts[depth]}, ${depth===parts.length-1?expr(value):update(depth+1)})`;}
      return `{% set ${root} = ${update(0)} %}`;
    }
    const indexed = name.match(/^(\w+)\[(.*)\]$/);
    if (indexed) return `{% set ns.${indexed[1]} = put(ns.${indexed[1]} | default({}), ${expr(indexed[2])}, ${expr(value)}) %}`;
    if (index) return `{% set ns.${name} = put(ns.${name} | default({}), ${expr(index)}, ${expr(value)}) %}`;
    return `{% set ${local(name)?name:'ns.'+name} = ${expr(value)} %}`;
  }
  let result = '{% set ns = namespace(keys=keys | default({}), contact_groups=contact_groups | default({})) %}', cursor = 0, textStart = 0;
  const literal = text => {
    if(options.xml)text=text.replace(/&copy;/g,'©').replace(/&nbsp;/g,' ').replace(/&(?!amp;|lt;|gt;|quot;|apos;|#\d+;|#x[\da-f]+;)/gi,'&amp;');
    return text.includes('{%')||text.includes('{{')||text.includes('{#')?`{% raw %}${text}{% endraw %}`:text;
  };
  while (cursor < source.length) {
    const start = source.indexOf('{', cursor);
    if (start < 0) break;
    const rest = source.slice(start);
    if (rest.startsWith('{*')) {
      const end = source.indexOf('*}', start + 2);
      if (end < 0) throw Error('Unclosed Smarty comment');
      result += literal(source.slice(textStart,start)); cursor=end+2; textStart=cursor; continue;
    }
    if (rest.startsWith('{literal}')) {
      const end = source.indexOf('{/literal}',start+9);
      if (end < 0) throw Error('Unclosed literal');
      result += literal(source.slice(textStart,start)) + literal(source.slice(start+9,end));
      cursor=end+10; textStart=cursor; continue;
    }
    if (!/^\{(?:\$|\(\$|if\s|elseif\s|else\}|\/if\}|foreach\s|\/foreach\}|for\s|\/for\}|assign\s|append\s|break\}|strip\}|\/strip\}|substr\(|count\(|math\s|["'])/.test(rest)) { cursor=start+1; continue; }
    let end=start+1,depth=1,quote='';
    for (;end<source.length;end++) {
      const ch=source[end];
      if (quote) { if(ch===quote && source[end-1]!=='\\') quote=''; }
      else if('"\''.includes(ch)) quote=ch;
      else if(ch==='{') depth++;
      else if(ch==='}' && --depth===0) break;
    }
    if(depth) throw Error(`Unclosed Smarty tag at ${start}: ${source.slice(start,start+100)}`);
    const tag=source.slice(start+1,end).trim();
    let output;
    if(tag.startsWith('if ')) output=`{% if ${expr(tag.slice(3))} %}`;
    else if(tag.startsWith('elseif ')) output=`{% elif ${expr(tag.slice(7))} %}`;
    else if(tag==='else') output='{% else %}';
    else if(tag==='/if') output='{% endif %}';
    else if(tag==='break') output='{% break %}';
    else if(tag==='strip'||tag==='/strip') output='';
    else if(tag==='/foreach'||tag==='/for') {output='{% endfor %}';scopes.pop();}
    else if(tag.startsWith('foreach ')) {
      const m=tag.replace(/\s+name\s*=\s*["']?\w+["']?\s*$/,'').match(/^foreach\s+(.+?)\s+as\s+\$(\w+)(?:\s*=>\s*\$(\w+))?$/);
      if(!m) throw Error(`Unsupported loop ${tag}`);
      const loopValue=['true','false','none'].includes(m[3])?'_loop_'+m[3]:m[3];
      output=`{% for ${m[2]}${m[3]?', '+loopValue:''} in ${expr(m[1])}${m[3]?' | items':' | values'} %}`;
      scopes.push([m[2],m[3]].filter(Boolean));
    } else if(tag.startsWith('for ')) {
      const m=tag.match(/^for\s+\$(\w+)\s*=\s*(.+?)\s+to\s+(.+?)(?:\s+step\s+(.+))?$/);
      if(!m)throw Error(`Unsupported for ${tag}`);
      output=`{% for ${m[1]} in range(${expr(m[2])}, (${expr(m[3])}) + 1${m[4]?', '+expr(m[4]):''}) %}`;
      scopes.push([m[1]]);
    } else if(tag.startsWith('assign ')||tag.startsWith('append ')) {
      const a=attributes(tag); if(!a.var||!a.value)throw Error(`Invalid assignment ${tag}`);
      output=assignment(a.var,a.value,a.index);
    } else if(tag.startsWith('math ')) {
      const a=attributes(tag);let equation=unquote(a.equation??'');
      for(const [name,value]of Object.entries(a).filter(([key])=>!['assign','equation','format'].includes(key))) equation=equation.replace(new RegExp(`\\b${name}\\b`,'g'),`(${value})`);
      output=a.assign?assignment(a.assign,equation):`{{ (${expr(equation)}) | int }}`;
    } else if(/^\$\w+\+\+$/.test(tag)){
      const name=tag.slice(1,-2);assigned.add(name);output=`{{ (${expr('$'+name)}) | output }}${assignment(name,'$'+name+'+1')}`;
    } else if(/^\$\w+(?:(?:\.\w+)|(?:\[[^\]]+\]))*\s*=(?!=)/.test(tag)) {
      const m=tag.match(/^\$(\w+(?:(?:\.\w+)|(?:\[[^\]]+\]))*)\s*=(?!=)\s*([\s\S]+)$/);output=assignment(m[1],m[2]);
    } else if(/^["']/.test(tag)&&tag.includes('{$')) {
      const parts=split(tag,'|');let value=unquote(parts.shift());
      const chunks=value.split(/(\{\$[^}]+\})/).filter(Boolean).map(part=>part.startsWith('{$')?expr(part.slice(1,-1)):JSON.stringify(part));
      output=`{{ (${chunks.join(' ~ ')})${parts.length?' | '+parts.join(' | '):''} | output }}`;
    } else output=`{{ (${expr(tag)}) | output }}`;
    result += literal(source.slice(textStart,start))+output;
    cursor=end+1;textStart=cursor;
  }
  return result+literal(source.slice(textStart));
}
