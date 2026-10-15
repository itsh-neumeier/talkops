// Backup targets (ADR 0029): types and helpers shared by the settings page
// and the restore assistant.

export type TargetKind = 'local' | 'smb' | 's3' | 'ftp' | 'webdav' | 'sftp';
export const targetKinds: TargetKind[] = ['smb', 's3', 'webdav', 'sftp', 'ftp', 'local'];

/** Keep in sync with `S3_PROVIDERS` in talkops-core. */
export const s3Providers = [
	'AWS',
	'Ceph',
	'Cloudflare',
	'DigitalOcean',
	'Hetzner',
	'IDrive',
	'IONOS',
	'Minio',
	'OVHcloud',
	'Rclone',
	'Scaleway',
	'SeaweedFS',
	'Synology',
	'Wasabi',
	'Other'
] as const;
/** Keep in sync with `WEBDAV_VENDORS` in talkops-core. */
export const webdavVendors = ['nextcloud', 'owncloud', 'infinitescale', 'rclone', 'other'] as const;

export type TargetConfig =
	| { kind: 'local' }
	| { kind: 'smb'; host: string; port: number | null; user: string; domain: string }
	| {
			kind: 's3';
			provider: string;
			endpoint: string;
			region: string;
			access_key_id: string;
			path_style: boolean;
	  }
	| {
			kind: 'ftp';
			host: string;
			port: number | null;
			user: string;
			tls: 'explicit' | 'implicit' | 'none';
	  }
	| { kind: 'webdav'; url: string; vendor: string; user: string }
	| { kind: 'sftp'; host: string; port: number | null; user: string; host_keys: string };

export type TargetSecret = {
	password?: string;
	secret_access_key?: string;
	private_key?: string;
};

export type Schedule = { weekdays: number; hour: number; minute: number; every_hours: number };
export type Retention = {
	keep_last: number;
	keep_daily: number;
	keep_weekly: number;
	keep_monthly: number;
	keep_yearly: number;
};

export type Activity = {
	action: 'backup' | 'fetch';
	file: string | null;
	started_at: string;
	error: string | null;
	done: boolean;
};

export type BackupTarget = {
	id: string;
	name: string;
	enabled: boolean;
	config: TargetConfig;
	path: string;
	allow_insecure: boolean;
	include_recordings: boolean;
	schedule: Schedule;
	retention: Retention;
	bwlimit_kbps: number | null;
	has_secret: boolean;
	last_run_at: string | null;
	last_success_at: string | null;
	last_file: string | null;
	last_error: string | null;
	created_at: string;
	next_run_at: string | null;
	activity: Activity | null;
};

export type TargetsOverview = {
	targets: BackupTarget[];
	encryption_configured: boolean;
	roots: string[];
};

export type Entry = { name: string; is_dir: boolean; size: number; modified: string | null };
export type BrowseResult = {
	entries: Entry[];
	truncated: boolean;
	host_keys: string | null;
	fingerprint: string | null;
};
export type TestReport = { host_keys: string | null; fingerprint: string | null };

/** A connection as the editor sends it (`DraftConnection` in the API). */
export type Connection = {
	id?: string;
	config: TargetConfig;
	path: string;
	allow_insecure: boolean;
	secret?: TargetSecret;
};

/** Default settings of a protocol. */
export function defaultConfig(kind: TargetKind): TargetConfig {
	switch (kind) {
		case 'local':
			return { kind };
		case 'smb':
			return { kind, host: '', port: null, user: '', domain: '' };
		case 's3':
			return {
				kind,
				provider: 'AWS',
				endpoint: '',
				region: '',
				access_key_id: '',
				path_style: false
			};
		case 'ftp':
			return { kind, host: '', port: null, user: '', tls: 'explicit' };
		case 'webdav':
			return { kind, url: '', vendor: 'nextcloud', user: '' };
		case 'sftp':
			return { kind, host: '', port: null, user: '', host_keys: '' };
	}
}

export const defaultSchedule = (): Schedule => ({
	weekdays: 127,
	hour: 3,
	minute: 30,
	every_hours: 0
});
export const defaultRetention = (): Retention => ({
	keep_last: 3,
	keep_daily: 7,
	keep_weekly: 4,
	keep_monthly: 12,
	keep_yearly: 0
});

/** Weekday bit (0 = Monday ... 6 = Sunday). */
export function hasDay(mask: number, day: number): boolean {
	return (mask & (1 << day)) !== 0;
}
export function toggleDay(mask: number, day: number): number {
	return mask ^ (1 << day);
}

/** Run times of one day, `HH:MM`. */
export function runTimes(s: Schedule): string[] {
	const out: string[] = [];
	const step = s.every_hours * 60;
	for (let at = s.hour * 60 + s.minute; at < 24 * 60; at += step) {
		out.push(`${String(Math.floor(at / 60)).padStart(2, '0')}:${String(at % 60).padStart(2, '0')}`);
		if (step === 0) break;
	}
	return out;
}

/** Joins a folder and an entry name of the browser. */
export function joinPath(base: string, name: string): string {
	if (!base) return name;
	return `${base.replace(/\/+$/, '')}/${name.replace(/^\/+/, '')}`;
}

/** The parent of a folder; `''` above the first level. */
export function parentPath(path: string, kind: TargetKind, roots: string[] = []): string {
	const trimmed = path.replace(/\/+$/, '');
	if (kind === 'local' && roots.some((r) => r.replace(/\/+$/, '') === trimmed)) return '';
	const i = trimmed.lastIndexOf('/');
	if (i <= 0) return kind === 'sftp' && i === 0 && trimmed.length > 1 ? '/' : '';
	return trimmed.slice(0, i);
}

/** TalkOps archive names, as `backup::valid_name` in the server. */
export function isArchive(name: string): boolean {
	return /^talkops-\d{8}-\d{6}\.tar\.gz(\.age)?$/.test(name);
}

/** Leaves out empty secret fields: missing fields keep the stored values. */
export function secretChange(secret: TargetSecret): TargetSecret | undefined {
	const out: TargetSecret = {};
	for (const [k, v] of Object.entries(secret) as [keyof TargetSecret, string | undefined][]) {
		if (v) out[k] = v;
	}
	return Object.keys(out).length ? out : undefined;
}

/** The address shown in the list, e.g. `smb://nas.local/talkops`. */
export function describe(config: TargetConfig, path: string): string {
	switch (config.kind) {
		case 'local':
			return path;
		case 'smb':
			return `smb://${config.host}/${path}`;
		case 's3':
			return `s3://${path}`;
		case 'ftp':
			return `ftp://${config.host}/${path}`;
		case 'webdav':
			return path ? `${config.url}/${path}` : config.url;
		case 'sftp':
			return `sftp://${config.user}@${config.host}${path.startsWith('/') ? '' : '/~/'}${path}`;
	}
}
