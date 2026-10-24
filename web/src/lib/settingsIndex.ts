// Search index of the settings, like the search of Android settings. The
// entries reuse the i18n keys of the pages, so results follow the language
// and the names of the UI. `fields` are labels on the target; `kw` adds
// synonyms in German and English that are not in the UI text.
import type { Permission } from './api.ts';
import type { MessageKey } from './i18n/index.svelte.ts';

/** Where an entry leads: a settings section (with tab) or a setup page. */
export interface SettingsTarget {
	section?: string;
	tab?: string;
	href?: string;
	/** Heading on the target to scroll to and mark. */
	heading?: MessageKey;
}

export interface SettingsEntry extends SettingsTarget {
	id: string;
	title: MessageKey;
	/** Section or page label for the path line. */
	parent: MessageKey | null;
	icon: string;
	/** One of these is necessary; empty = everyone. */
	perms: Permission[];
	fields?: MessageKey[];
	kw?: string;
}

type Sub = {
	title: MessageKey;
	tab?: string;
	tabLabel?: MessageKey;
	perms?: Permission[];
	fields?: MessageKey[];
	kw?: string;
};

/** Headings inside the sections of the settings page. */
const SECTION_ENTRIES: Record<string, Sub[]> = {
	telephony: [
		{
			title: 'settings.dialing',
			fields: [
				'settings.countryCode',
				'settings.areaCode',
				'settings.nationalPrefix',
				'settings.internationalPrefix',
				'settings.emergency',
				'settings.defaultNumber',
				'settings.timezone',
				'settings.externalIp',
				'settings.sipUriDialing'
			],
			kw: 'vorwahl ortsvorwahl landesvorwahl notruf 110 112 nat stun zeitzone dial plan wählen'
		}
	],
	calls: [
		{
			title: 'rec.title',
			kw: 'aufnahme aufzeichnung mitschnitt recording record löschfrist retention'
		},
		{
			title: 'stt.profilesTitle',
			perms: ['transcription.settings'],
			kw: 'transkription whisper speech to text spracherkennung live'
		},
		{
			title: 'stt.providersTitle',
			perms: ['transcription.settings'],
			kw: 'openai api key schlüssel anbieter provider cloud'
		},
		{
			title: 'stt.usageTitle',
			perms: ['transcription.settings'],
			kw: 'kosten cost usage verbrauch'
		},
		{ title: 'music.title', kw: 'wartemusik hold music warteschleife musik' },
		{ title: 'callMap.settingsTitle', kw: 'karte map herkunft origin ortsnetz' }
	],
	voicemail: [
		{ title: 'vmc.keys', kw: 'mailbox anrufbeantworter tasten keys pin' },
		{ title: 'vmc.announce', kw: 'ansage announcement begrüßung greeting' },
		{ title: 'vmc.voices', kw: 'stimme voice piper tts sprachausgabe' },
		{ title: 'vmc.texts', kw: 'texte texts ansagetext' }
	],
	email: [
		{ title: 'smtp.title', kw: 'mail server smtp e-mail versand absender' },
		{ title: 'mailDesign.title', kw: 'logo farbe color design branding' },
		{ title: 'mailDesign.templates', kw: 'vorlage template betreff subject' }
	],
	fax: [
		{ title: 'fax.title', kw: 't38 telefax' },
		{ title: 'fax.storageTitle', kw: 'speicher ablage ordner folder' },
		{ title: 'fax.mailInput', kw: 'mail to fax e-mail eingang imap' },
		{ title: 'fax.whitelist', kw: 'whitelist erlaubt allowlist absender' },
		{ title: 'fax.endpoints' },
		{ title: 'fax.inputLog', kw: 'protokoll log' },
		{ title: 'fax.mailReports', kw: 'bericht report sendebericht' }
	],
	notifications: [
		{ title: 'notif.title', kw: 'alarm alert benachrichtigung warnung trunk ausfall' },
		{ title: 'notif.log', kw: 'protokoll log verlauf' }
	],
	security: [
		{
			title: 'identity.title',
			perms: ['identity.manage'],
			kw: 'ldap active directory ad oidc sso single sign-on anmeldung login keycloak entra'
		},
		{
			title: 'guard.title',
			perms: ['security.manage'],
			kw: 'sip guard fail2ban sperre block angriff attack scanner'
		},
		{ title: 'guard.bans', perms: ['security.manage'], kw: 'gesperrt banned ip sperrliste' }
	],
	system: [
		{
			title: 'diag.overview',
			tab: 'status',
			tabLabel: 'system.tab.status',
			kw: 'status version freeswitch'
		},
		{ title: 'diag.trunks', tab: 'status', tabLabel: 'system.tab.status' },
		{
			title: 'diag.registrations',
			tab: 'status',
			tabLabel: 'system.tab.status',
			kw: 'registrierung registration geräte'
		},
		{
			title: 'diag.channels',
			tab: 'status',
			tabLabel: 'system.tab.status',
			kw: 'kanäle channels gespräche'
		},
		{
			title: 'diag.capture',
			tab: 'diagnostics',
			tabLabel: 'system.tab.diagnostics',
			kw: 'sip trace log debug fehlersuche protokoll'
		},
		{
			title: 'diag.sessions',
			tab: 'sessions',
			tabLabel: 'system.tab.sessions',
			kw: 'sitzungen sessions angemeldet logout'
		},
		{
			title: 'diag.restart',
			tab: 'maintenance',
			tabLabel: 'system.tab.maintenance',
			kw: 'neustart restart wartung reload'
		}
	],
	backup: [
		{ title: 'backup.title', kw: 'sicherung backup datensicherung zeitplan schedule' },
		{ title: 'backup.configTitle', kw: 'konfiguration export' },
		{ title: 'backup.portainer.title', kw: 'portainer docker stack' },
		{ title: 'backup.importTitle', kw: 'wiederherstellen restore import' },
		{ title: 'targets.title', kw: 'ziel nas smb sftp s3 webdav' }
	],
	storage: [{ title: 'storage.title', kw: 'speicher nfs smb nas aufnahmen ordner' }],
	account: [
		{ title: 'settings.account', kw: 'passwort password kennwort ändern' },
		{ title: 'mfa.title', kw: '2fa totp zwei faktor two factor authenticator otp' }
	]
};

/** Keywords and sub-entries of the setup pages. */
const PAGE_KEYWORDS: Record<string, string> = {
	'/me': 'meine telefone my phones softphone geräte',
	'/extensions': 'nebenstelle durchwahl extension rufumleitung forwarding dnd geräte devices',
	'/users': 'benutzer user rollen roles rechte permissions konto',
	'/routing':
		'anrufsteuerung smart attendant ivr warteschlange queue gruppe group zeitsteuerung time condition sperrliste blocklist spam',
	'/trunks': 'sip trunk provider anbieter telekom leitung gateway registrierung',
	'/numbers': 'rufnummer number did msn ziel destination',
	'/phones': 'yealink provisionierung provisioning firmware klingelton ringtone tasten keys',
	'/door-stations': 'tür sprechanlage dahua vto video klingel doorbell',
	'/wifi-surveys': 'wlan wifi begehung survey roaming handset',
	'/audit': 'protokoll log änderungen audit'
};

const PAGE_SUBS: Record<string, Sub[]> = {
	'/wifi-surveys': [
		{
			title: 'wifi.tabUnifi',
			tab: 'unifi',
			perms: ['unifi.manage'],
			kw: 'ubiquiti unifi api schlüssel key konsole console voip ssid'
		},
		{ title: 'wifi.tabChanges', tab: 'changes', perms: ['unifi.manage'], kw: 'rollback verlauf' }
	],
	'/audit': [
		{ title: 'audit.retention', kw: 'löschfrist aufbewahrung retention' },
		{ title: 'audit.export', kw: 'export syslog siem' }
	]
};

/** Builds the index from the sections and pages of the settings navigation. */
export function buildIndex(
	sections: { id: string; icon: string; perms: Permission[] }[],
	pages: { href: string; key: MessageKey; icon: string; perm: Permission | null }[]
): SettingsEntry[] {
	const out: SettingsEntry[] = [];
	for (const s of sections) {
		const label = `settings.section.${s.id}` as MessageKey;
		out.push({ id: s.id, title: label, parent: null, icon: s.icon, perms: s.perms, section: s.id });
		for (const sub of SECTION_ENTRIES[s.id] ?? []) {
			out.push({
				id: `${s.id}:${sub.title}`,
				title: sub.title,
				parent: label,
				icon: s.icon,
				perms: sub.perms ?? s.perms,
				section: s.id,
				tab: sub.tab,
				heading: sub.title,
				fields: [...(sub.tabLabel ? [sub.tabLabel] : []), ...(sub.fields ?? [])],
				kw: sub.kw
			});
		}
	}
	for (const p of pages) {
		const perms = p.perm ? [p.perm] : [];
		out.push({
			id: p.href,
			title: p.key,
			parent: 'settings.setup',
			icon: p.icon,
			perms,
			href: p.href,
			kw: PAGE_KEYWORDS[p.href]
		});
		for (const sub of PAGE_SUBS[p.href] ?? []) {
			out.push({
				id: `${p.href}:${sub.title}`,
				title: sub.title,
				parent: p.key,
				icon: p.icon,
				perms: sub.perms ?? perms,
				href: sub.tab ? `${p.href}?tab=${sub.tab}` : p.href,
				heading: sub.tab ? undefined : sub.title,
				kw: sub.kw
			});
		}
	}
	return out;
}

/**
 * Text for matching: lower case, without accents; ä/ae, ö/oe, ü/ue and
 * ß/ss compare equal.
 */
export function fold(text: string): string {
	return text
		.toLowerCase()
		.replace(/ß/g, 'ss')
		.normalize('NFD')
		.replace(/\p{M}/gu, '')
		.replace(/ae/g, 'a')
		.replace(/oe/g, 'o')
		.replace(/ue/g, 'u');
}

const words = (text: string) =>
	fold(text)
		.split(/[^\p{L}\p{N}]+/u)
		.filter(Boolean);

export interface SearchHit {
	entry: SettingsEntry;
	score: number;
}

/**
 * Entries that match each word of `query` (as a word start). The title
 * counts more than the path, field labels and keywords.
 */
export function searchSettings(
	query: string,
	entries: SettingsEntry[],
	label: (key: MessageKey) => string,
	allowed: (perms: Permission[]) => boolean,
	limit = 8
): SearchHit[] {
	const tokens = words(query);
	if (tokens.length === 0) return [];
	const hits: SearchHit[] = [];
	for (const entry of entries) {
		if (!allowed(entry.perms)) continue;
		const title = words(label(entry.title));
		const rest = [
			...(entry.parent ? words(label(entry.parent)) : []),
			...(entry.fields ?? []).flatMap((f) => words(label(f))),
			...words(entry.kw ?? '')
		];
		let score = 0;
		for (const token of tokens) {
			if (title.some((w) => w === token)) score += 4;
			else if (title.some((w) => w.startsWith(token))) score += 3;
			else if (rest.some((w) => w.startsWith(token))) score += 1;
			else if (token.length >= 4 && title.some((w) => w.includes(token))) score += 1;
			else {
				score = 0;
				break;
			}
		}
		if (score > 0) hits.push({ entry, score: score - title.length * 0.01 });
	}
	return hits.sort((a, b) => b.score - a.score).slice(0, limit);
}

/** URL of an entry. */
export function entryUrl(entry: SettingsTarget): string {
	if (entry.href) return entry.href;
	const query = new URLSearchParams({ section: entry.section ?? '' });
	if (entry.tab) query.set('tab', entry.tab);
	return `/settings?${query}`;
}
