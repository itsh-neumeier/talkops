// Firmware boundary, hardware status, TFTP delivery, resync and ringtones in the device settings.
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
            ringtone_max_kb: enterprise ? 160 : 0,
            provisioning_capabilities: { ...model.provisioning_capabilities, resync: enterprise, reboot: enterprise, delivery: enterprise ? 'tftp' : 'http',
                settings: enterprise ? [{ key: 'cisco_enterprise_tftp_ip', label: 'TFTP client IPv4', kind: 'text', copyable: false }] : [] } };
        const selectedPhone = { ...phone, model: selectedModel.id, delivery: enterprise ? 'tftp' : 'http',
            provisioning: enterprise ? { vendor_settings: { cisco_enterprise_tftp_ip: '192.0.2.10' } } : {},
            provisioning_files: enterprise ? [{ artifact: 'SEP{$mac|upper}.cnf.xml', format: 'xml', name: 'SEP805EC0010203.cnf.xml', url: '', primary: true },
                { artifact: 'dialplan.xml', format: 'xml', name: 'dialplan.xml', url: '', primary: false }] : [] };
        const fields = enterprise ? [{ key: 'f1', parameter: 'device[0]/devicePool[0]/dateTimeSetting[0]/dateTemplate[0]', description: '', artifact: 'SEP{$mac|upper}.cnf.xml',
            category: 'locale', kind: 'text', managed: true, copyable: false, default: null }] : [];
        const hintPattern = enterprise ? (language === 'de' ? /Experimentelles Enterprise-SIP/ : /Experimental Enterprise SIP/) : (language === 'de' ? /Nur MPP-\/3PCC-Firmware/ : /MPP \/ 3PCC firmware only/);
        const context = await browser.newContext({ viewport: { width: 768, height: 1000 } });
        await context.addInitScript(lang => { localStorage.setItem('talkops.locale', lang); }, language);
        let saved = null, resync = null;
        await context.route('**/api/v1/**', async route => {
            const path = new URL(route.request().url()).pathname;
            let value = [];
            if (route.request().method() === 'POST' && path === '/api/v1/phones/cisco/resync') {
                resync = route.request().postDataJSON();
                return route.fulfill({ json: { notified: 1 } });
            }
            if (path === '/api/v1/phone-media') return route.fulfill({ json: [{ id: 'ring', kind: 'ringtone', name: 'Bells', filename: 'talkops-ring.wav', size_bytes: 64000, uploaded_at: '2026-10-29T12:00:00Z' }] });
            if (route.request().method() === 'PUT' && path === '/api/v1/phones/cisco') {
                saved = route.request().postDataJSON();
                return route.fulfill({ json: { ...selectedPhone, provisioning: saved.provisioning } });
            }
            if (path === '/api/v1/auth/me') value = { user: { id: 'admin', username: 'admin', display_name: 'Admin', role: 'admin', enabled: true }, csrf_token: 'fixture', permissions: [] };
            else if (path === '/api/v1/status') value = { version: 'test', database: { ok: true }, freeswitch: { ok: true } };
            else if (path === '/api/v1/phone-models') value = [selectedModel];
            else if (path === '/api/v1/phones') value = [selectedPhone];
            else if (path === '/api/v1/phones/cisco') value = selectedPhone;
            else if (path.endsWith('/parameters')) value = { fields };
            else if (path === '/api/v1/phone-settings') value = { catalog: [], values: {} };
            await route.fulfill({ json: value });
        });
        const page = await context.newPage();
        await page.goto(base + '/phones/cisco');
        const hint = page.getByText(hintPattern);
        await hint.waitFor();
        assert.match(await hint.innerText(), /Enterprise/);
        if (!enterprise) {
            assert.match(await hint.innerText(), language === 'de' ? /ungetestet/ : /Untested/);
            assert.equal(await page.getByRole('button', { name: /Resync/ }).count(), 0);
        }
        if (enterprise) {
            assert.match(await hint.innerText(), /TFTP/);
            assert.match(await hint.innerText(), language === 'de' ? /CP-7861 mit Firmware 14\.2\(1\) geprüft/ : /passed on a CP-7861 with firmware 14\.2\(1\)/);
            // TFTP delivery: file names and the client address, no HTTPS URL or credential note.
            const files = page.locator('section', { has: page.getByRole('heading', { name: language === 'de' ? 'Konfigurationsdateien' : 'Configuration files' }) });
            const body = await files.innerText();
            assert.match(body, /SEP805EC0010203\.cnf\.xml/);
            assert.match(body, /192\.0\.2\.10/);
            assert.match(body, language === 'de' ? /DHCP-Option 150/ : /DHCP option 150/);
            assert.doesNotMatch(body, language === 'de' ? /Passwort für die Provisionierung sind erforderlich/ : /password are required/);
            assert.doesNotMatch(body, /https:\/\//);
            // Ringtones are offered, with the note to select the tone on the phone.
            await page.locator('#ph-ring').selectOption('ring');
            assert.match(await page.locator('body').innerText(), language === 'de' ? /Einstellungen → Klingelton/ : /Settings → Ringtone/);
            // Resync and restart send service-control through the resync endpoint.
            await page.getByRole('button', { name: /Resync/, exact: false }).first().click();
            for (let i = 0; i < 50 && !resync; i++) await page.waitForTimeout(100);
            assert.deepEqual(resync, { reboot: false });
            // XML paths get readable labels and the real file name.
            await page.locator('summary').filter({ hasText: language === 'de' ? 'Sprache, Datum' : 'Language, date' }).click();
            await page.getByText('dateTimeSetting › dateTemplate', { exact: true }).waitFor();
            await page.getByText('SEP805EC0010203.cnf.xml', { exact: true }).first().waitFor();
            await page.locator('summary').filter({ hasText: language === 'de' ? 'Provisionierung' : 'Provisioning' }).click();
            const ip = page.getByLabel(language === 'de' ? 'TFTP-Client-IPv4-Adresse (feste DHCP-Zuordnung)' : 'TFTP client IPv4 address (fixed DHCP lease)', { exact: true });
            await ip.fill('192.0.2.20');
            assert.equal(await ip.inputValue(), '192.0.2.20');
            // The TFTP exporter only reads the stored value, so the save request must carry it.
            await page.getByRole('button', { name: language === 'de' ? 'Speichern' : 'Save', exact: true }).click();
            for (let i = 0; i < 50 && !saved; i++) await page.waitForTimeout(100);
            assert.equal(saved?.provisioning?.vendor_settings?.cisco_enterprise_tftp_ip, '192.0.2.20');
            assert.equal(saved?.ringtone_id, 'ring');
        }
        await page.goto(base + '/phones');
        await page.getByRole('button', { name: language === 'de' ? 'Telefon hinzufügen' : 'Add phone', exact: true }).click();
        await page.locator('#p-model').selectOption(selectedModel.id);
        await page.getByRole('dialog').getByText(hintPattern).waitFor({timeout:5000}).catch(async error => { console.error((await page.getByRole('dialog').innerText()).slice(-5000)); throw error; });
        await context.close();
    }
    console.log('Cisco MPP and Enterprise firmware boundaries, hardware status, TFTP files, resync, ringtones and saved TFTP fields passed in DE and EN.');
} finally { await browser.close(); }
