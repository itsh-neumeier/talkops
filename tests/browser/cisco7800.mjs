// Firmware boundary and untested status in the model selector and device settings.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({ headless: true });
const base = process.env.BROWSER_BASE_URL || 'http://127.0.0.1:5184';
const model = { id: 'cisco-7861-mpp', name: '7861 MPP / 3PCC', vendor: 'cisco', family: 'desk', accounts: 16,
    line_keys: 0, line_key_limit: 0, layout: null, video: false, wallpaper: false, ringtone_max_kb: 0,
    provisioning_capabilities: { native: true, adaptation: 'adapted', device_validation: 'not_tested',
        identity_kind: 'mac', resync: false, reboot: false, settings: [], key_groups: [], key_functions: [] } };
const phone = { id: 'cisco', name: 'Cisco office', model: model.id, mac: '805ec0010203', accounts: [], extensions: [],
    line_keys: [], settings: {}, provisioning: {}, phonebook_sections: [], ringtone_id: null, wallpaper_id: null, last_seen_at: null, last_ip: null, last_firmware: null, provisioning_files: [], created_at: '2026-10-28T12:00:00Z' };
try {
    for (const variant of ['mpp', 'enterprise']) for (const language of ['de', 'en']) {
        const enterprise = variant === 'enterprise';
        const selectedModel = { ...model, id: `cisco-7861-${variant}`, name: enterprise ? '7861 Enterprise SIP (experimental)' : model.name, accounts: enterprise ? 1 : 16,
            provisioning_capabilities: { ...model.provisioning_capabilities, settings: enterprise ? [{ key: 'cisco_enterprise_tftp_ip', label: 'TFTP client IPv4', kind: 'text', copyable: false }] : [] } };
        const selectedPhone = { ...phone, model: selectedModel.id };
        const hintPattern = enterprise ? (language === 'de' ? /Experimentelles Enterprise-SIP/ : /Experimental Enterprise SIP/) : (language === 'de' ? /Nur MPP-\/3PCC-Firmware/ : /MPP \/ 3PCC firmware only/);
        const context = await browser.newContext({ viewport: { width: 768, height: 1000 } });
        await context.addInitScript(lang => { localStorage.setItem('talkops.locale', lang); }, language);
        let saved = null;
        await context.route('**/api/v1/**', async route => {
            const path = new URL(route.request().url()).pathname;
            let value = [];
            if (route.request().method() === 'PUT' && path === '/api/v1/phones/cisco') {
                saved = route.request().postDataJSON();
                return route.fulfill({ json: { ...selectedPhone, provisioning: saved.provisioning } });
            }
            if (path === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture', permissions: [] };
            else if (path === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
            else if (path === '/api/v1/phone-models') value = [selectedModel];
            else if (path === '/api/v1/phones') value = [selectedPhone];
            else if (path === '/api/v1/phones/cisco') value = selectedPhone;
            else if (path.endsWith('/parameters')) value = { fields: [] };
            else if (path === '/api/v1/phone-settings') value = { catalog: [], values: {} };
            await route.fulfill({ json: value });
        });
        const page = await context.newPage();
        await page.goto(base + '/phones/cisco');
        const hint = page.getByText(hintPattern);
        await hint.waitFor();
        assert.match(await hint.innerText(), /Enterprise/);
        assert.match(await hint.innerText(), language === 'de' ? /ungetestet/ : /Untested/);
        assert.equal(await page.getByRole('button', { name: /Resync/ }).count(), 0);
        if (enterprise) {
            assert.match(await hint.innerText(), /TFTP/);
            await page.locator('summary').filter({ hasText: language === 'de' ? 'Provisionierung' : 'Provisioning' }).click();
            const ip = page.getByLabel(language === 'de' ? 'TFTP-Client-IPv4-Adresse (feste DHCP-Zuordnung)' : 'TFTP client IPv4 address (fixed DHCP lease)', { exact: true });
            await ip.fill('192.0.2.20');
            assert.equal(await ip.inputValue(), '192.0.2.20');
            // The TFTP exporter only reads the stored value, so the save request must carry it.
            await page.getByRole('button', { name: language === 'de' ? 'Speichern' : 'Save', exact: true }).click();
            for (let i = 0; i < 50 && !saved; i++) await page.waitForTimeout(100);
            assert.equal(saved?.provisioning?.vendor_settings?.cisco_enterprise_tftp_ip, '192.0.2.20');
        }
        await page.goto(base + '/phones');
        await page.getByRole('button', { name: language === 'de' ? 'Telefon hinzufügen' : 'Add phone', exact: true }).click();
        await page.locator('#p-model').selectOption(selectedModel.id);
        await page.getByRole('dialog').getByText(hintPattern).waitFor({timeout:5000}).catch(async error => { console.error((await page.getByRole('dialog').innerText()).slice(-5000)); throw error; });
        await context.close();
    }
    console.log('Cisco MPP and Enterprise firmware boundaries, untested status and saved TFTP fields passed in DE and EN.');
} finally { await browser.close(); }
