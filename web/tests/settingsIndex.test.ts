import assert from 'node:assert/strict';
import test from 'node:test';
import de from '../src/lib/i18n/de.ts';
import en from '../src/lib/i18n/en.ts';
import {
	buildIndex,
	entryUrl,
	fold,
	searchSettings,
	type SettingsEntry
} from '../src/lib/settingsIndex.ts';

// Same shape as `sections` and `settingsPages` in SettingsNav.svelte.
const sections = [
	{ id: 'telephony', icon: '☎', perms: ['settings.manage'] },
	{ id: 'calls', icon: '🎵', perms: ['settings.manage'] },
	{ id: 'voicemail', icon: '📮', perms: ['voicemail.settings'] },
	{ id: 'email', icon: '✉', perms: ['voicemail.settings'] },
	{ id: 'fax', icon: '📠', perms: ['fax.manage'] },
	{ id: 'notifications', icon: '🔔', perms: ['notifications.manage'] },
	{ id: 'security', icon: '🛡', perms: ['identity.manage', 'security.manage'] },
	{ id: 'system', icon: '⚙', perms: ['diagnostics.manage'] },
	{ id: 'backup', icon: '💾', perms: ['backups.manage'] },
	{ id: 'storage', icon: '🗄', perms: ['storage.manage'] },
	{ id: 'account', icon: '👤', perms: [] }
] as never[];
const pages = [
	{ href: '/me', key: 'nav.myPhones', icon: '📱', perm: null },
	{ href: '/extensions', key: 'nav.extensions', icon: '☏', perm: 'extensions.view' },
	{ href: '/trunks', key: 'nav.trunks', icon: '🔌', perm: 'trunks.view' },
	{ href: '/wifi-surveys', key: 'nav.wifiSurvey', icon: '📶', perm: 'wifi_survey.run' },
	{ href: '/audit', key: 'nav.audit', icon: '📜', perm: 'audit.view' }
] as never[];
const index = buildIndex(sections, pages);
const all = () => true;
const titles = (q: string, dict: Record<string, string> = de, allowed = all) =>
	searchSettings(q, index, (k) => dict[k as keyof typeof dict], allowed).map((h) => h.entry.title);

test('each key of the index exists in both languages', () => {
	const keys = index.flatMap((e: SettingsEntry) => [
		e.title,
		...(e.parent ? [e.parent] : []),
		...(e.heading ? [e.heading] : []),
		...(e.fields ?? [])
	]);
	for (const key of keys) {
		assert.ok(key in en, `en: ${key}`);
		assert.ok(key in de, `de: ${key}`);
	}
	assert.equal(new Set(index.map((e) => e.id)).size, index.length, 'unique ids');
});

test('folds case, accents and German umlaut spellings', () => {
	assert.equal(fold('Größe'), fold('groesse'));
	assert.equal(fold('Übersicht'), fold('uebersicht'));
	assert.equal(fold('ÄNDERN'), 'andern');
});

test('finds titles, field labels and synonyms in both languages', () => {
	assert.equal(titles('wartemusik')[0], 'music.title');
	assert.equal(titles('hold music', en)[0], 'music.title');
	assert.equal(titles('ldap')[0], 'identity.title');
	assert.equal(titles('vorwahl')[0], 'settings.dialing');
	assert.equal(titles('ubiquiti')[0], 'wifi.tabUnifi');
	assert.equal(titles('zwei faktor')[0], 'mfa.title');
	assert.ok(titles('sicher').includes('backup.title'));
	assert.deepEqual(titles('   '), []);
	assert.deepEqual(titles('xyzzy'), []);
});

test('hides entries without permission', () => {
	const none = (perms: string[]) => perms.length === 0;
	assert.deepEqual(titles('ldap', de, none), []);
	assert.equal(titles('passwort', de, none)[0], 'settings.account');
});

test('builds the URL of an entry', () => {
	assert.equal(
		entryUrl({ section: 'system', tab: 'diagnostics' }),
		'/settings?section=system&tab=diagnostics'
	);
	assert.equal(entryUrl({ section: 'fax' }), '/settings?section=fax');
	assert.equal(entryUrl({ href: '/wifi-surveys?tab=unifi' }), '/wifi-surveys?tab=unifi');
});
