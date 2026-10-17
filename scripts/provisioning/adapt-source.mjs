// Build-time corrections. The checked-in reference blobs are never modified.
export function adaptSource(vendor,path,input){
    let source=input;
    // These stray markers are present in the pinned Flyingvoice source blobs.
    // Keep the originals for attribution, but never send them to a device.
    source=source.replace(/^>>>>>>> Stashed changes\r?\n?/gm,'');
    // An explicit empty leaf has the same XML meaning and exposes an editable
    // value range to the manufacturer settings form.
    source=source.replace(/<([A-Za-z_][\w:.-]*)(\s[^<>]*?)?\s*\/>/g,(_,name,attributes)=>`<${name}${attributes??''}></${name}>`);
    source=source.replace(/\$account\.(\d+)\.assword\b/g,'$account.$1.password')
        .replace(/\$account\.(\d+)\.(user_id|auth_id)_\1\b/g,'$account.$1.$2');
    if(vendor==='fanvil'){
        // The X7C source confuses key placement with key function. Use the
        // same function codes as the other Fanvil profiles. TalkOps slots are
        // one-based, including the account number carried by a line key.
        source=source.replace(/\$k_category\s*==\s*'line'/g,"$k_type == '1'")
            .replace(/\$k_category\s*==\s*'memory'/g,"$k_type != '3'")
            .replace(/\$row\.device_key_category\s*==\s*'line'/g,"$row.device_key_type == '1'")
            .replace(/\$row\.device_key_category\s*==\s*'memory'/g,"$row.device_key_type != '3'")
            .replace(/\$row\.device_key_line\s*\+\s*1/g,'$row.device_key_line');
    }
    if(vendor==='grandstream'&&/^grandstream\/(?:gxp110x|gxp2100)\//.test(path)){
        const bindings={271:['1','enabled'],270:['1','label'],47:['1','server_address'],35:['1','user_id'],36:['1','auth_id'],34:['1','password'],3:['1','display_name']};
        for(let account=2;account<=4;account++)for(const [suffix,field]of [[1,'enabled'],[17,'label'],[2,'server_address'],[4,'user_id'],[5,'auth_id'],[6,'password'],[7,'display_name']])bindings[200+account*100+suffix]=[String(account),field];
        // Account two uses P4xx, account three P5xx, and account four P6xx.
        for(const [tag,[account,field]]of Object.entries(bindings)){
            const pattern=new RegExp(`<P${tag}>(?:${field==='enabled'?'[01]':''})</P${tag}>`,'g');
            const value=field==='enabled'?`{if isset($account.${account}.password)}1{else}0{/if}`:`{$account.${account}.${field}}`;
            source=source.replace(pattern,`<P${tag}>${value}</P${tag}>`);
        }
    }
    // Several copied Yealink account blocks use the user ID as the password.
    if(vendor==='yealink')source=source.replace(/(account\.(\d+)\.password\s*=\s*)\{\$account\.\2\.user_id\}/g,'$1{$account.$2.password}');
    if(path==='atcom/ag198/{$mac}.cfg') {
        const values={'SIP1 Phone Number':'{$account.1.user_id}','SIP1 Display Name':'{$account.1.display_name}','SIP1 Sip Name':'{$account.1.user_id}',
            'SIP1 Register Addr':'{$account.1.server_address}','SIP1 Register Port':'{$account.1.sip_port}','SIP1 Register User':'{$account.1.auth_id}',
            'SIP1 Register Pwd':'{$account.1.password}','SIP1 Enable Reg':'{if isset($account.1.password)}1{else}0{/if}',
            'SIP1 Proxy Addr':'{$account.1.server_address}','SIP1 Proxy Port':'{$account.1.sip_port}','SIP1 Proxy User':'{$account.1.auth_id}',
            'SIP1 Proxy Pwd':'{$account.1.password}','SIP1 Local Domain':'{$account.1.server_address}',
            'SNTP Server':'{$ntp_server_primary}','DHCP Dns':'1','Primary DNS':'','Alter DNS':''};
        source=source.replace(/^([^:\r\n]+):(.*)$/gm,(match,name)=>Object.hasOwn(values,name.trim())?`${name}:${values[name.trim()]}`:match);
    }
    if(vendor==='mitel')source=source.replace(/<admin_passwd>[0-9a-f]{32}<\/admin_passwd>/gi,'<admin_passwd>{$admin_password|md5}</admin_passwd>');
    if(path==='polycom/650/custom.cfg')source=source.replace(/(efk\.efklist\.1\.action\.string="[^"\r\n]*")\s*(<\/efk>)/,'$1 />\n$2');
    if(['poly','polycom'].includes(vendor))source=source.replace(/(voIpProt\.SIP\.alertInfo\.(\d+)\.value="[^"]*"\s+voIpProt\.SIP\.alertInfo\.)\d+(\.class=)/g,'$1$2$3');
    if(path==='grandstream/ht814/{$mac}.xml')source=source.replace('<P192>{$grandstream_firmware_path}/ht814/{$firmware_version}</P192>','{if isset($grandstream_firmware_path) && isset($firmware_version)}<P192>{$grandstream_firmware_path}/ht814/{$firmware_version}</P192>')
        .replace('<P192>{$domain_name}{$project_path}/app/provision</P192>','<P192>{$domain_name}{$project_path}/app/provision</P192>{/if}');
    // Replace URLs before Smarty conversion, so TLS, port, path and credentials
    // all come from the effective TalkOps endpoint rather than the SIP host.
    source=source.replace(/(?:https?:\/\/)?(?:\{if\s+isset\(\$http_auth_username\)\})?\{\$http_auth_username\}:\{\$http_auth_password\}@(?:\{\/if\})?\{\$(?:domain_name|row\.server_address|account\.1\.server_address)\}(?:\{\$project_path\})?\/app\/provision\/?/g,'{$artifact_base}/')
        .replace(/(?:https?:\/\/)?\{\$(?:domain_name|row\.server_address|account\.1\.server_address)\}(?:\{\$project_path\})?\/app\/provision\/?/g,'{$artifact_base}/')
        .replace(/\{\$artifact_base\}\/(?:index\.php)?\?(?:(?:mac|address)=\{\$mac\}(?:&amp;|&))?file=([\w.-]+)(?:(?:&amp;|&)(?:mac|address)=\{\$mac\})?((?:&amp;|&)?)/g,(_,file,separator)=>`{$artifact_base}/${file}${separator?'?':''}`)
        .replace(/\{\$artifact_base\}\/\{\$mac\}\//g,'{$artifact_base}/')
        .replace(/\{\$artifact_base\}\/(?:index\.php)?\?(?:mac|address)=\{\$mac\}/g,'{$primary_url}')
        .replace(/\{\$artifact_base\}\/\{literal\}\$?\{mac\}\{\/literal\}\.xml/g,'{$primary_url}');
    // These optional upstream URLs have no corresponding source artifact.
    // Keep the field editable, but do not enable a guaranteed 404 by default.
    if(vendor==='grandstream')source=source.replace(/\{\$artifact_base\}\/resources\/firmware\//g,'{$grandstream_firmware_path}');
    if(vendor==='snom')source=source.replace(/(<language\b[^>]*>)English(<\/language>)/g,'$1{$snom_language}$2');
    if(vendor==='snom')source=source.replace(/(<firmware_status\b[^>]*>)\{\$(?:artifact_base|primary_url)\}[^<]*(<\/firmware_status>)/g,'$1$2');
    if(vendor==='swissvoice')source=source.replace(/^(XML-PBook[123] Addr\s*:).*$/gm,'$1');
    if(vendor==='panasonic')source=source.replace(/https?:\/\/\{\$server_address_1\}\{\$project_path\}\/app\/provision/g,'{$primary_url}');
    if(vendor==='algo')source=source.replace(/^prov\.download\.cfgpath\s*=.*$/gm,'prov.download.cfgpath = {$provision_path}')
        .replace(/^prov\.server\.static\s*=.*$/gm,'prov.server.static = {$provision_host}')
        .replace(/^prov\.auth\.pwd\s*=.*$/gm,'prov.auth.pwd = {$http_auth_password}')
        .replace(/^prov\.auth\.user\s*=.*$/gm,'prov.auth.user = {$http_auth_username}');
    if(vendor==='groundwire'&&path.endsWith('qr_template.txt'))source='{$groundwire_link}';
    if(path==='avaya/J100Supgrade.txt')source='GET {$mac}.cfg\n';
    return source;
}
