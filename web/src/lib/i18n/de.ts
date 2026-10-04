import type en from './en';

const de: Record<keyof typeof en, string> = {
	'app.tagline': 'Selbst gehostete Telefonanlage',
	'status.title': 'Systemstatus',
	'status.version': 'Version',
	'status.database': 'Datenbank',
	'status.freeswitch': 'FreeSWITCH',
	'status.ok': 'OK',
	'status.down': 'Nicht erreichbar',
	'status.loading': 'Lädt…',
	'status.error': 'Die TalkOps-API ist nicht erreichbar.',
	'theme.toggle': 'Dunkelmodus umschalten',
	'locale.label': 'Sprache'
};

export default de;
