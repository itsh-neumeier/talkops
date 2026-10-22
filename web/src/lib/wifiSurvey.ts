// Wi-Fi voice survey (ADR 0041): types of the API and small helpers.

export type SurveyState =
	'armed' | 'calling' | 'running' | 'analysing' | 'analysed' | 'failed' | 'cancelled';

export type Rating = 'clean' | 'audible' | 'bad';

export interface Thresholds {
	clean_ms: number;
	bad_ms: number;
	loss_block_pct: number;
	sticky_rssi: number;
	sticky_secs: number;
	pingpong_count: number;
	pingpong_secs: number;
	weak_rssi: number;
	weak_secs: number;
	retention_days: number;
	max_minutes: number;
	max_parallel: number;
}

export interface Pattern {
	kind:
		| 'sticky_client'
		| 'pingpong'
		| 'drop_without_roam'
		| 'roam_without_drop'
		| 'weak_coverage'
		| 'vad_suspected';
	start_ms: number;
	end_ms: number;
	ap?: string;
	other_ap?: string;
	value?: number;
}

export interface Recommendation {
	id: string;
	severity: 'info' | 'warning' | 'critical';
	values: Record<string, unknown>;
}

export interface ApShare {
	ap_mac: string;
	ap_name: string | null;
	seconds: number;
	avg_rssi: number | null;
	min_rssi: number | null;
}

export interface Summary {
	duration_ms: number;
	roams: number;
	clean: number;
	audible: number;
	bad: number;
	band_changes: number;
	worst_gap_ms: number;
	loss_pct: number;
	mos: number | null;
	min_rssi: number | null;
	avg_rssi: number | null;
	markers: number;
	drops_without_roam: number;
	vad_suspected: boolean;
	has_radio: boolean;
	aps: ApShare[];
	patterns: Pattern[];
	recommendations: Recommendation[];
}

export interface Survey {
	id: string;
	device_id: string | null;
	extension_id: string | null;
	phone_id: string | null;
	device_label: string;
	mac: string | null;
	label: string;
	mode: 'originate' | 'self_dial';
	state: SurveyState;
	call_uuid: string | null;
	ptime_ms: number;
	codec: string | null;
	recording_secs: number | null;
	started_by: string | null;
	armed_until: string | null;
	started_at: string | null;
	answered_at: string | null;
	ended_at: string | null;
	hangup_cause: string | null;
	thresholds: Thresholds;
	unifi: UnifiState;
	summary: Summary | null;
	error: string | null;
	expires_at: string;
	created_at: string;
}

export interface Roam {
	offset_ms: number;
	from_ap_mac: string | null;
	from_ap_name: string | null;
	to_ap_mac: string | null;
	to_ap_name: string | null;
	from_radio: string | null;
	to_radio: string | null;
	from_channel: number | null;
	to_channel: number | null;
	rssi_before: number | null;
	rssi_after: number | null;
	gap_ms: number;
	loss_pct: number;
	jitter_ms: number | null;
	rating: Rating;
	flags: string[];
	markers: number;
}

export interface MediaSample {
	offset_ms: number;
	interval_ms: number;
	received: number;
	expected: number;
	flaws: number;
	mos: number | null;
	jitter_max_ms: number | null;
	rtcp_fraction_lost: number | null;
	rtcp_jitter_ms: number | null;
}

export interface RadioSample {
	offset_ms: number;
	ap_mac: string | null;
	ap_name: string | null;
	bssid: string | null;
	radio: string | null;
	channel: number | null;
	rssi: number | null;
	tx_rate_kbps: number | null;
	rx_rate_kbps: number | null;
	error: string | null;
}

export interface SurveyEvent {
	id: string;
	offset_ms: number;
	kind: 'marker' | 'error_period' | 'state';
	marker: 'disturbance' | 'location' | 'note' | null;
	source: 'dtmf' | 'ui' | null;
	note: string;
	data: Record<string, unknown>;
}

export interface Timeline {
	media: MediaSample[];
	radio: RadioSample[];
	events: SurveyEvent[];
}

export interface LiveNow {
	offset_ms: number;
	received: number | null;
	expected: number | null;
	loss_10s_pct: number;
	gap_10s_ms: number;
	rating_5s: Rating | null;
	mos: number | null;
	rtcp_fraction_lost: number | null;
	rtcp_jitter_ms: number | null;
	radio: RadioSample | null;
}

export interface Live {
	survey: Survey;
	now: LiveNow;
	timeline: Timeline;
	analysis: { roams: Roam[]; summary: Summary };
}

export interface SurveyDevice {
	device_id: string;
	extension_id: string;
	extension_number: string;
	extension_name: string;
	device_name: string;
	enabled: boolean;
	phone_id: string | null;
	phone_model: string | null;
	mac: string | null;
}

export interface UnifiState {
	status?: 'not_configured' | 'no_mac' | 'connected' | 'not_found' | 'error';
	version?: string | null;
	ap_mac?: string | null;
	ap_name?: string | null;
	radio?: string | null;
	channel?: number | null;
	rssi?: number | null;
	error?: string | null;
}

export interface UnifiConnection {
	url: string;
	site: string;
	cert_sha256: string;
	version: string | null;
	capabilities: {
		version?: string | null;
		integration?: boolean;
		classic?: boolean;
		site_id?: string | null;
		access_points?: number;
	};
	last_check_at: string | null;
	last_error: string | null;
}

export interface PairComparison {
	from_ap: string;
	to_ap: string;
	a_count: number;
	b_count: number;
	a_avg_gap_ms: number | null;
	b_avg_gap_ms: number | null;
	a_worst: Rating | null;
	b_worst: Rating | null;
}

export const ACTIVE: SurveyState[] = ['armed', 'calling', 'running'];

/** `mm:ss` (or `h:mm:ss`) of a survey offset. */
export function clock(ms: number): string {
	const total = Math.max(0, Math.floor(ms / 1000));
	const h = Math.floor(total / 3600);
	const m = Math.floor((total % 3600) / 60);
	const s = total % 60;
	const mm = String(m).padStart(h ? 2 : 1, '0');
	return `${h ? `${h}:` : ''}${mm}:${String(s).padStart(2, '0')}`;
}

/** Badge class of a rating. */
export function ratingBadge(r: Rating | null | undefined): string {
	return `badge ${r === 'bad' ? 'badge-bad' : r === 'audible' ? 'badge-warn' : r ? 'badge-ok' : 'badge-muted'}`;
}

/** Badge class of a survey state. */
export function stateBadge(s: SurveyState): string {
	if (s === 'running' || s === 'calling' || s === 'armed') return 'badge badge-warn';
	if (s === 'analysed') return 'badge badge-ok';
	if (s === 'failed') return 'badge badge-bad';
	return 'badge badge-muted';
}

/** UniFi radio code as band. */
export function band(radio: string | null | undefined): string {
	if (radio === 'ng') return '2.4 GHz';
	if (radio === 'na') return '5 GHz';
	if (radio === '6e') return '6 GHz';
	return radio ?? '—';
}

/** Name of an AP, or its MAC. */
export function apLabel(name: string | null | undefined, mac: string | null | undefined): string {
	return name || mac || '—';
}

/** Signal quality class of an RSSI value (dBm). */
export function rssiLevel(rssi: number | null | undefined): Rating | null {
	if (rssi == null) return null;
	if (rssi >= -67) return 'clean';
	if (rssi >= -75) return 'audible';
	return 'bad';
}

/** Mbit/s from kbit/s. */
export function mbit(kbps: number | null | undefined): string {
	return kbps == null ? '—' : (kbps / 1000).toFixed(kbps >= 100_000 ? 0 : 1);
}

/** Appends new live values to a timeline (offsets are unique). */
export function mergeTimeline(into: Timeline, more: Timeline): Timeline {
	const lastMedia = into.media.at(-1)?.offset_ms ?? -1;
	const lastRadio = into.radio.at(-1)?.offset_ms ?? -1;
	const known = new Set(into.events.map((e) => e.id));
	return {
		media: [...into.media, ...more.media.filter((m) => m.offset_ms > lastMedia)],
		radio: [...into.radio, ...more.radio.filter((r) => r.offset_ms > lastRadio)],
		events: [...into.events, ...more.events.filter((e) => !known.has(e.id))]
	};
}

/** Newest offset of a timeline (for the next live request). */
export function newestOffset(tl: Timeline): number {
	return Math.max(-1, tl.media.at(-1)?.offset_ms ?? -1, tl.radio.at(-1)?.offset_ms ?? -1);
}
