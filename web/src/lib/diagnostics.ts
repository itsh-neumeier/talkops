// Shared types and helpers of the system diagnostics views
// (Settings → System: status, debug logging, sessions, maintenance).

export type Capture = {
	running: boolean;
	level: string | null;
	sip_trace: boolean;
	started_at: string | null;
	until: string | null;
	lines: number;
	last_seq: number;
};

export type Channel = {
	uuid: string;
	created: string;
	name: string;
	state: string;
	callstate: string;
	cid_name: string;
	cid_num: string;
	dest: string;
	application: string;
	application_data: string;
	read_codec: string;
	write_codec: string;
	secure: string;
};

/** `GET /diagnostics`. */
export type DiagnosticsView = {
	version: string;
	server_started_at: string;
	freeswitch: string | null;
	capture: Capture;
	channels: Channel[];
};

export type LogLine = { seq: number; text: string };

/** Time since `iso` as "3d 4h 12m", "4h 12m" or "12m". */
export function since(iso: string, now: number): string {
	const secs = Math.max(0, Math.floor((now - Date.parse(iso)) / 1000));
	const d = Math.floor(secs / 86400);
	const h = Math.floor((secs % 86400) / 3600);
	const m = Math.floor((secs % 3600) / 60);
	return d ? `${d}d ${h}h ${m}m` : h ? `${h}h ${m}m` : `${m}m`;
}

/** FreeSWITCH "UP 0 years, 3 days, 4 hours, 12 minutes, …" → "3d 4h 12m". */
export function freeswitchUptime(status: string | null | undefined): string {
	const first = status?.split('\n')[0] ?? '';
	const n = (unit: string) => Number(first.match(new RegExp(`(\\d+) ${unit}`))?.[1] ?? 0);
	const days = n('years') * 365 + n('days');
	const h = n('hours');
	const m = n('minutes');
	return days ? `${days}d ${h}h ${m}m` : h ? `${h}h ${m}m` : `${m}m`;
}
