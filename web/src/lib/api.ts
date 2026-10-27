import { requestFinished, requestStarted } from './net.svelte.ts';
// Typed client for the TalkOps REST API (same origin, cookie session).

export class ApiError extends Error {
	constructor(
		public status: number,
		public code: string,
		message: string,
		/** The full JSON error body, e.g. extra fields of a 409. */
		public body: Record<string, unknown> = {}
	) {
		super(message);
	}
}

let csrfToken = '';

export function setCsrf(token: string) {
	csrfToken = token;
}

/** Called when the session is gone (401); set by the session store. */
let onUnauthorized: () => void = () => {};
export function setUnauthorizedHandler(fn: () => void) {
	onUnauthorized = fn;
}

export async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
	if (method !== 'GET') return send<T>(method, path, body);
	requestStarted();
	try {
		return await send<T>(method, path, body);
	} finally {
		requestFinished();
	}
}

async function send<T>(method: string, path: string, body?: unknown): Promise<T> {
	return (await sendWithHeaders<T>(method, path, body)).data;
}

async function sendWithHeaders<T>(
	method: string,
	path: string,
	body?: unknown
): Promise<{ data: T; headers: Headers }> {
	const headers: Record<string, string> = { 'X-Requested-With': 'TalkOps' };
	if (csrfToken) headers['X-CSRF-Token'] = csrfToken;
	if (body !== undefined) headers['Content-Type'] = 'application/json';
	const res = await fetch(`/api/v1${path}`, {
		method,
		headers,
		body: body === undefined ? undefined : JSON.stringify(body),
		credentials: 'same-origin'
	});
	if (res.status === 204) return { data: undefined as T, headers: res.headers };
	const data = await res.json().catch(() => ({}));
	if (!res.ok) {
		if (res.status === 401 && !path.startsWith('/auth/login') && path !== '/auth/me')
			onUnauthorized();
		throw new ApiError(res.status, data.error ?? 'error', data.message ?? res.statusText, data);
	}
	return { data: data as T, headers: res.headers };
}

/** A GET of a list with the number of all matches in `X-Total-Count`. */
async function getPage<T>(path: string): Promise<{ items: T[]; total: number }> {
	requestStarted();
	try {
		const { data, headers } = await sendWithHeaders<T[]>('GET', path);
		const header = headers.get('X-Total-Count');
		const total = header === null ? NaN : Number(header);
		return { items: data, total: Number.isFinite(total) ? total : data.length };
	} finally {
		requestFinished();
	}
}

export const api = {
	getPage,
	get: <T>(path: string) => request<T>('GET', path),
	post: <T>(path: string, body?: unknown) => request<T>('POST', path, body ?? {}),
	put: <T>(path: string, body: unknown) => request<T>('PUT', path, body),
	del: (path: string) => request<void>('DELETE', path),
	/** DELETE that returns a JSON body. */
	delJson: <T>(path: string) => request<T>('DELETE', path)
};

// --- types (mirror the Rust API) -------------------------------------------

export type Role = 'admin' | 'operator' | 'user';

/** Permission keys (see talkops_core::roles). */
export type Permission =
	| 'fax.send'
	| 'fax.mail_submit'
	| 'fax.templates.edit'
	| 'fax.use_global'
	| 'fax.access_all'
	| 'fax.delete'
	| 'fax.manage'
	| 'calls.active'
	| 'calls.view_all'
	| 'calls.live_transcript'
	| 'calls.live_transcript_all'
	| 'recordings.access_all'
	| 'recordings.delete'
	| 'recordings.retranscribe'
	| 'transcriptions.view'
	| 'transcriptions.manage'
	| 'voicemail.access_all'
	| 'voicemail.settings'
	| 'phonebook.edit'
	| 'extensions.view'
	| 'extensions.manage'
	| 'softphone.any_extension'
	| 'users.view'
	| 'users.manage'
	| 'routing.view'
	| 'routing.manage'
	| 'routing.override'
	| 'blocking.manage'
	| 'blocking.allowlist'
	| 'blocking.settings'
	| 'dialing.manage'
	| 'trunks.view'
	| 'trunks.manage'
	| 'phones.view'
	| 'phones.manage'
	| 'doors.manage'
	| 'settings.manage'
	| 'transcription.settings'
	| 'security.manage'
	| 'identity.manage'
	| 'backups.manage'
	| 'storage.manage'
	| 'diagnostics.manage'
	| 'notifications.manage'
	| 'audit.view'
	| 'audit.manage'
	| 'wifi_survey.run'
	| 'wifi_survey.manage'
	| 'unifi.manage'
	| 'unifi.write';

export interface PermissionInfo {
	key: Permission;
	/** Group in the editor, e.g. `calls` or `system`. */
	area: string;
	/** Gives access to secrets, the system or more rights. */
	sensitive: boolean;
}

export interface RoleRecord {
	id: string;
	/** Built-in role, or null for an own role. */
	builtin: Role | null;
	name: string;
	description: string;
	/** Empty for the admin role, which has all permissions. */
	permissions: Permission[];
	directory_group: string;
	users: number;
	updated_at: string;
}

export interface RolesOverview {
	permissions: PermissionInfo[];
	roles: RoleRecord[];
}

export interface User {
	id: string;
	username: string;
	display_name: string;
	email: string | null;
	role: Role;
	/** Own role; null: the built-in role in `role`. */
	role_id: string | null;
	enabled: boolean;
	auth_source: string;
	last_login_at: string | null;
	totp_enabled: boolean;
}

export interface Me {
	user: User;
	csrf_token: string;
	/** Permission keys of the user's role; admins get all keys. */
	permissions: Permission[];
}

export interface Component {
	ok: boolean;
	detail?: string;
}

export interface SystemStatus {
	version: string;
	database: Component;
	freeswitch: Component;
}

export interface Extension {
	id: string;
	number: string;
	display_name: string;
	user_id: string | null;
	outbound_number_id: string | null;
	hide_caller_id: boolean;
	ring_timeout_secs: number;
	enabled: boolean;
	dnd: boolean;
	forward_all: string | null;
	record_calls: 'inherit' | 'always' | 'never';
	/** Internal video calls (both sides need it). */
	video_enabled: boolean;
	/** Own dialing profile (ADR 0045); null: the default profile. */
	dialing_profile_id?: string | null;
}

export type DeviceKind =
	'desk' | 'dect' | 'wifi' | 'softphone' | 'mobile' | 'door' | 'other' | 'browser';

export interface Device {
	id: string;
	extension_id: string;
	browser_user_id: string | null;
	name: string;
	kind: DeviceKind;
	sip_username: string;
	phone_id: string | null;
	account_index: number | null;
	/** Label on the provisioned phone; empty = extension number. */
	phone_label: string;
	/** Caller name sent by the provisioned phone; empty = display name. */
	phone_display_name: string;
	/** Codec profile; null = automatic from the device kind. */
	codec_profile: CodecProfile | null;
	/** Video on the device's calls; null = automatic from the codec profile. */
	video: DeviceVideo | null;
	/** Format of the next generated SIP password; never changes the current one. */
	sip_password_format: SipPasswordFormat;
	enabled: boolean;
}

/** Generated SIP passwords: 20 or 15 letters and digits. */
export type SipPasswordFormat = 'standard' | 'compact15';

/** Codecs TalkOps uses with a device (ADR 0023). */
export type CodecProfile = 'standard' | 'hd' | 'robust' | 'g711';

/** Video codecs of a device on calls with video (ADR 0028). */
export type DeviceVideo = 'auto' | 'off' | 'h264' | 'vp8';

/** Video setting in use: the configured one, else `auto` with the standard
 * codec profile and `off` with every other profile. */
export function effectiveDeviceVideo(
	profile: CodecProfile,
	configured: DeviceVideo | null
): DeviceVideo {
	return configured ?? (profile === 'standard' ? 'auto' : 'off');
}

/** Profile in use: the configured one, else the default of the device kind. */
export function effectiveCodecProfile(
	kind: DeviceKind,
	configured: CodecProfile | null
): CodecProfile {
	return configured ?? (kind === 'wifi' ? 'hd' : 'standard');
}

export interface ExtensionWithDevices extends Extension {
	devices: Device[];
}

export interface DeviceCredentials {
	device_id: string;
	sip_username: string;
	sip_password: string;
	sip_domain: string;
}

export interface LocalizedText {
	en: string;
	de: string;
}

export interface SipSettings {
	registrar?: string | null;
	realm?: string | null;
	proxy?: string | null;
	outbound_proxy?: string | null;
	transport: 'udp' | 'tcp' | 'tls';
	srtp: 'off' | 'optional' | 'required';
	register: boolean;
	number_format: string;
	caller_id_format: string;
	caller_id_header: string;
}

export interface Preset {
	id: string;
	name: string;
	product: string;
	country: string;
	status: 'verified' | 'community' | 'untested';
	sources: string[];
	notes: LocalizedText;
	credentials: {
		mode: 'per_number' | 'shared' | 'no_registration';
		username_template: string;
		username_hint: LocalizedText;
	};
	sip: SipSettings;
}

export interface Trunk {
	id: string;
	name: string;
	preset: string;
	overrides: Record<string, unknown>;
	enabled: boolean;
	/** Offer video to the provider. */
	video_enabled: boolean;
}

export interface GatewayState {
	name: string;
	state: string;
	status: string;
	last_error: string | null;
}

export interface TrunkAccount {
	id: string;
	trunk_id: string;
	username: string;
	auth_username: string;
	enabled: boolean;
	/** Inbound calls to the account itself (accounts without numbers). */
	destination_type: DestinationType;
	destination_id: string | null;
	gateway: string;
	state: GatewayState | null;
}

export interface PhoneNumber {
	id: string;
	trunk_id: string;
	account_id: string | null;
	e164: string;
	label: string;
	destination_type: DestinationType;
	destination_id: string | null;
	/** Further extensions ringing together with `destination_id` (the main one). */
	extra_extensions: string[];
	enabled: boolean;
}

export interface TrunkDetail extends Trunk {
	accounts: TrunkAccount[];
	numbers: PhoneNumber[];
}

export interface Settings {
	country_code: string;
	area_code: string;
	national_prefix: string;
	international_prefix: string;
	emergency_numbers: string[];
	external_ip: string;
	default_language: string;
	default_number_id: string | null;
	timezone: string;
	record_inbound: boolean;
	record_outbound: boolean;
	record_internal: boolean;
	recording_announcement: boolean;
	recording_retention_days: number;
	transcription_enabled: boolean;
	/** Extensions may dial SIP addresses (user@domain). */
	sip_uri_dialing: boolean;
	/** Names and terms Whisper should recognise, comma-separated. */
	transcription_vocabulary: string;
	/** Built-in piece, '' for all pieces shuffled. */
	hold_music: string;
	/** Own music on hold (takes precedence). */
	hold_music_clip_id: string | null;
}

export interface Registration {
	user: string;
	network_ip: string;
	network_port: string;
	transport: string;
	user_agent: string;
	expires: number;
}

export interface LiveStatus {
	connected: boolean;
	registrations: Registration[];
	gateways: Record<string, GatewayState>;
	updated_at: string | null;
}

export interface Call {
	id: string;
	call_uuid: string;
	direction: 'inbound' | 'outbound' | 'internal';
	caller_number: string;
	caller_name: string;
	destination: string;
	extension_id: string | null;
	started_at: string;
	answered_at: string | null;
	ended_at: string;
	duration_secs: number;
	billsec: number;
	hangup_cause: string;
	blocked_reason?: string | null;
	/** Current whitelist membership, only supplied to whitelist managers. */
	allowlisted?: boolean;
	recording_id: string | null;
	dest_extension_id?: string | null;
	number_id?: string | null;
	/** Resolved caller (extension or phone book name). */
	caller?: CallParty;
	/** Resolved called party; the answering extension of inbound calls. */
	called?: CallParty;
	/** Own number: dialed (inbound) or caller ID (outbound). */
	via_number?: string | null;
}

export type StatsRange = '1h' | '1d' | '1w' | '1m';

/** Calls of one category group (call origin map). */
export interface MapCounts {
	answered: number;
	missed: number;
	outbound: number;
	blocked: number;
}

/** A bubble of the call origin map, or an entry without a location. */
export interface MapArea {
	/** Geo area ID (`DE-951`), `<country>-none` (mobile or no location) or `none`. */
	id: string;
	/** `area`: a local network area or place; `none`: no location below the country. */
	kind: 'area' | 'none';
	/** Device shape; optional for cached responses from earlier servers. */
	marker_kind?: 'area' | 'mobile' | 'unlocated';
	/** ISO country code; `null` when the country is unknown. */
	country: string | null;
	name: string;
	/** Display prefix, e.g. `+49 951`. */
	prefix: string;
	/** Bubble position; `null` when the country is unknown. */
	lat: number | null;
	lon: number | null;
	counts: MapCounts;
	total: number;
	/** Talk time of all calls in seconds. */
	billsec: number;
	/** Reasons of entries without a location. */
	mobile: number;
	service: number;
	unresolved: number;
	/** Newest calls first, at most 20. */
	calls: Call[];
}

export interface GeoSource {
	file: string;
	title: string;
	source: string;
	license: string;
	updated: string;
}

/** Optional tile server of the call map; an empty URL selects the offline map. */
export interface MapTiles {
	tile_url: string;
	attribution: string;
}

export interface CallMap {
	range: StatsRange;
	since: string;
	until: string;
	/** More calls than the limit: the oldest calls are not counted. */
	truncated: boolean;
	limit: number;
	counts: MapCounts;
	total: number;
	/** Calls without a caller number (counted, not on the map). */
	anonymous: MapCounts;
	areas: MapArea[];
	sources: GeoSource[];
}

export interface CallStats {
	range: StatsRange;
	buckets: { start: string; inbound: number; outbound: number; internal: number; missed: number }[];
	end: string;
	total: number;
	inbound: number;
	outbound: number;
	internal: number;
	missed: number;
	blocked: number;
	answer_rate: number | null;
	avg_talk_secs: number;
	/** Totals of the period of the same length before. */
	previous?: {
		total: number;
		inbound: number;
		outbound: number;
		internal: number;
		missed: number;
		blocked: number;
		answer_rate: number | null;
		avg_talk_secs: number;
	};
}

export interface ActiveCall {
	uuid: string;
	caller_number: string;
	caller_name: string;
	destination: string;
	callee_number: string;
	callee_name: string;
	state: 'ringing' | 'talking' | 'held' | 'parked' | 'system';
	started_at: string | null;
	direction: Call['direction'] | null;
	caller: CallParty;
	called: CallParty;
	answered_at: string | null;
	transcript_available: boolean;
}

export interface CallParty {
	name: string;
	number: string;
	extension_id: string | null;
	/** Phone book entry that supplies `name`. */
	contact_id?: string | null;
	ambiguous: boolean;
	/** All phone book entries with this number when `ambiguous`. */
	matches?: ContactMatch[];
}

export interface ContactMatch {
	contact_id: string;
	name: string;
	/** Null for the global phone book. */
	section_id: string | null;
	section_name: string;
}

export interface LiveTranscript {
	id: string;
	started_at: string;
	processed_secs: number;
	segments: TranscriptSegment[];
	status: 'queued' | 'running' | 'done' | 'failed';
}

export type TranscriptStatus = 'none' | 'pending' | 'done' | 'failed';

export interface Recording {
	id: string;
	call_uuid: string;
	cdr_id: string | null;
	duration_secs: number;
	size_bytes: number;
	transcript_status: TranscriptStatus;
	created_at: string;
}

export interface TranscriptSegment {
	start: number;
	end: number;
	/** `caller`, `called` or empty (voicemail). */
	speaker: string;
	text: string;
}

export interface Transcript {
	id: string;
	recording_id: string | null;
	voicemail_id: string | null;
	language: string;
	text: string;
	segments: TranscriptSegment[];
	/** e.g. `whisper:large-v3-turbo-q5_0` or `api:whisper-1`. */
	engine: string;
	/** False while a more accurate second pass is still to come. */
	final: boolean;
	created_at: string;
}

export interface SearchHit {
	transcript_id: string;
	recording_id: string | null;
	voicemail_id: string | null;
	/** Matches are wrapped in `[` `]`. */
	snippet: string;
	created_at: string;
	caller_number: string | null;
	destination: string | null;
	rank: number;
}

export interface AuditEntry {
	id: number;
	user_id: string | null;
	username: string | null;
	action: string;
	entity_type: string;
	entity_id: string | null;
	details: Record<string, unknown>;
	ip: string | null;
	created_at: string;
}

export type KeyType =
	| 'conference'
	| 'transfer'
	| 'hold'
	| 'none'
	| 'line'
	| 'blf'
	| 'speed_dial'
	| 'forward'
	| 'dnd'
	| 'redial'
	| 'call_return'
	| 'pickup'
	| 'voicemail'
	| 'dtmf'
	| 'intercom'
	| 'xml_browser'
	| 'history'
	| 'directory'
	| 'menu'
	| 'status'
	| 'prefix'
	| 'local_phonebook'
	| 'xml_phonebook'
	| 'switch_account_up'
	| 'switch_account_down'
	| 'keypad_lock';

export interface LineKey {
	key: number;
	type: KeyType;
	value: string;
	label: string;
	account: number;
}

export interface Phone {
	provisioning?: PhoneProvisioning;
	id: string;
	mac: string;
	model: string;
	name: string;
	line_keys: LineKey[];
	last_seen_at: string | null;
	last_ip: string | null;
	last_firmware: string | null;
	ringtone_id: string | null;
	wallpaper_id: string | null;
	/** Phone book sections shown on the phone (at most 3). */
	phonebook_sections: string[];
	/** Comfort settings of this phone; unset keys use the values for all phones. */
	settings: Record<string, string>;
}

/** A phone in the list, with the extensions placed on it. */
export interface PhoneListItem extends Phone {
	extensions: { number: string; display_name: string }[];
}

/** A comfort setting from the phone catalog (key tone, display …). */
export interface PhoneSettingInfo {
	models?: string[];
	vendor?: string;
	key: string;
	group: string;
	kind: 'bool' | 'choice' | 'int';
	values: string[];
	/** The phone's factory value, if documented. */
	default: string | null;
	/** Limits of an `int`. */
	min?: number | null;
	max?: number | null;
	families: string[];
	param: string;
}

export interface PhoneSettingsView {
	catalog: PhoneSettingInfo[];
	/** Values for all phones. */
	values: Record<string, string>;
}

export interface PhoneAccount {
	account_index: number;
	device_id: string;
	extension_id: string;
	extension_number: string;
	display_name: string;
	phone_label: string;
	phone_display_name: string;
}

export interface PhoneDetail extends Phone {
	provisioning_base?: string | null;
	provisioning_files?: {
		artifact: string;
		format: string;
		name: string;
		url: string;
		primary: boolean;
	}[];
	accounts: PhoneAccount[];
}

export interface PhoneModel {
	provisioning_capabilities?: {
		key_groups?: string[];
		key_functions?: {
			uuid: string;
			type: string;
			subtype: string;
			value: string;
			description: string;
			groups?: string[];
		}[];
		native: boolean;
		adaptation: string;
		device_validation: string;
		account_capacity_source?: string;
		settings?: { key: string; label: string; kind: string; copyable: boolean }[];
		primary?: string;
		files?: string[];
		resync: boolean;
		reboot: boolean;
		identity_kind: 'mac' | 'token';
	};
	key_capabilities?: KeyCapabilities;
	id: string;
	name: string;
	vendor: string;
	family:
		'desk' | 'dect' | 'conference' | 'wifi' | 'ata' | 'gateway' | 'intercom' | 'paging' | 'app';
	accounts: number;
	line_keys: number;
	line_key_limit: number;
	layout: PhoneLayout | null;
	/** Line keys can be speed dials (not on Wi-Fi handsets). */
	speed_dial_keys: boolean;
	video: boolean;
	/** Largest custom ringtone in KiB; 0 = not supported. */
	ringtone_max_kb: number;
	wallpaper: boolean;
}

/** Row-major maps: zero denotes the page control. Touch maps are DSS overviews. */
export interface PhoneLayout {
	width: number;
	height: number;
	color: boolean;
	touch: boolean;
	left: number;
	right: number;
	columns: number;
	page_key: boolean;
	pages: number[][];
}

export type MediaKind = 'ringtone' | 'wallpaper';

/** An uploaded ringtone or wallpaper for provisioned phones. */
export interface PhoneMedia {
	id: string;
	kind: MediaKind;
	name: string;
	filename: string;
	size_bytes: number;
	uploaded_at: string;
}

export interface PhonebookSection {
	id: string;
	name: string;
}

export interface Firmware {
	id: string;
	model: string;
	filename: string;
	size_bytes: number;
	sha256: string;
	active: boolean;
	uploaded_at: string;
}

export interface Contact {
	id: string;
	name: string;
	company: string;
	phone_work: string;
	phone_mobile: string;
	phone_other: string;
	/** Phone book section; null = global (every phone). */
	section_id: string | null;
}

/** Result of a phone book CSV import (or its dry run). */
export interface ContactImportReport {
	created: number;
	updated: number;
	unchanged: number;
	skipped: number;
	sections_created: string[];
	failed: number;
	errors: { line: number; message: string }[];
	columns: string[];
}

export interface ProvisioningInfo {
	url: string;
	url_with_credentials: string;
	username: string;
	password: string;
	phone_admin_password: string;
}

export interface VoicemailBox {
	extension_id: string;
	enabled: boolean;
	has_pin: boolean;
	email_notify: boolean;
	attach_audio: boolean;
	language: 'de' | 'en' | null;
	greeting: 'default' | 'tts' | 'recorded' | 'clip' | 'none';
	greeting_text: string;
	greeting_status: 'none' | 'pending' | 'ready' | 'failed';
	greeting_clip_id: string | null;
	speech_speed: number;
	max_message_secs: number;
	/** How long the extension rings before voicemail answers. */
	ring_timeout_secs: number;
	new_messages: number;
	saved_messages: number;
}

export interface VoicemailMessage {
	id: string;
	extension_id: string;
	caller_number: string;
	caller_name: string;
	duration_secs: number;
	status: 'new' | 'saved';
	created_at: string;
	heard_at: string | null;
	transcript_status: TranscriptStatus;
}

export type TranscriptionPurpose =
	'live' | 'recording' | 'recording_refine' | 'voicemail' | 'voicemail_refine';
export type LocalModel = 'base' | 'fast' | 'accurate' | 'best' | 'german';

/** Engine of one purpose: off (second passes only), a local model or a cloud model. */
export interface TranscriptionProfile {
	purpose: TranscriptionPurpose;
	engine: 'off' | 'local' | 'cloud';
	local_model: LocalModel;
	provider_id: string | null;
	model: string;
	/** Live only: stream over the Realtime API. */
	realtime: boolean;
	/** Own price per audio minute; null = list price. */
	price_per_minute: number | null;
	/** Known list price of the cloud model (read-only). */
	list_price?: number | null;
}

/** Cloud connection: OpenAI or an OpenAI-compatible server (key never returned). */
export interface TranscriptionProvider {
	id: string;
	name: string;
	kind: 'openai' | 'compatible';
	url: string;
	has_key: boolean;
}

export interface TranscriptionUsage {
	month: string;
	purpose: TranscriptionPurpose;
	engine: string;
	seconds: number;
	cost: number;
	unpriced_seconds: number;
}

export interface TranscriptionOverview {
	providers: TranscriptionProvider[];
	profiles: TranscriptionProfile[];
	usage: TranscriptionUsage[];
	openai_prices: { model: string; price_per_minute: number }[];
}

export interface SmtpSettings {
	host: string;
	port: number;
	security: 'starttls' | 'tls' | 'none';
	username: string;
	has_password: boolean;
	from: string;
}

/** Mail branding (ADR 0034). */
export interface MailBranding {
	instance_name: string;
	sender_name: string;
	accent_color: string;
	header_text: string;
	footer_text: string;
	has_logo: boolean;
	logo_type: string | null;
	logo_width: number | null;
	logo_height: number | null;
}

export interface MailTemplateEntry {
	kind: string;
	language: 'de' | 'en';
	subject: string;
	body: string;
	custom: boolean;
	default_subject: string;
	default_body: string;
}

export interface MailDesignView {
	branding: MailBranding;
	instance: { name: string; url: string; hostname: string; version: string };
	kinds: { key: string; placeholders: string[] }[];
	templates: MailTemplateEntry[];
	common_placeholders: string[];
	max_logo_bytes: number;
}

export type NotificationSeverity = 'info' | 'warning' | 'critical';

export interface NotificationEventType {
	key: string;
	category: string;
	severity: NotificationSeverity;
	/** Problem events: the event that ends the problem. */
	recovery: string | null;
	/** False: no producer yet. */
	implemented: boolean;
}

export interface NotificationRule {
	event_type: string;
	enabled: boolean;
	min_severity: NotificationSeverity;
	recipient_users: string[];
	recipient_roles: string[];
	recipient_emails: string[];
	delay_secs: number;
	send_recovery: boolean;
	digest_secs: number;
	rate_limit_per_hour: number;
	/** True if changed from the defaults. */
	custom: boolean;
}

export interface NotificationsView {
	smtp_configured?: boolean;
	categories: string[];
	event_types: NotificationEventType[];
	rules: NotificationRule[];
}

export interface NotificationEvent {
	id: number;
	event_type: string;
	source: string;
	severity: NotificationSeverity;
	params: Record<string, unknown>;
	created_at: string;
}

export interface NotificationDelivery {
	id: number;
	event_type: string;
	kind: 'alert' | 'recovery' | 'digest' | 'test';
	subject: string;
	recipients: string[];
	status: string;
	error: string | null;
	event_count: number;
	created_at: string;
}

export interface NotificationLog {
	events: NotificationEvent[];
	deliveries: NotificationDelivery[];
}

export interface NotificationTestResult {
	status: string;
	recipients: string[];
	error: string | null;
}

export interface RenderedMail {
	subject: string;
	html: string;
	text: string;
}

export type DestinationType =
	'fax' | 'none' | 'extension' | 'voicemail' | 'ring_group' | 'time_condition' | 'ivr' | 'queue';

export interface RingGroup {
	id: string;
	number: string | null;
	name: string;
	strategy: 'simultaneous' | 'sequential';
	ring_timeout_secs: number;
	caller_id_prefix: string;
	fallback_type: DestinationType;
	fallback_id: string | null;
	enabled: boolean;
	members: string[];
}

export interface TimeConditionState {
	open: boolean;
	reason: 'override' | 'holiday' | 'closed_date' | 'schedule';
	holiday: string | null;
}

export interface TimeCondition {
	id: string;
	number: string | null;
	name: string;
	schedule: Record<string, [string, string][]>;
	holiday_region: string | null;
	closed_dates: string[];
	override: 'auto' | 'open' | 'closed';
	open_type: DestinationType;
	open_id: string | null;
	closed_type: DestinationType;
	closed_id: string | null;
	state: TimeConditionState;
}

export interface MenuOption {
	digit: string;
	type: DestinationType;
	id: string | null;
}

/** One step of a Smart Attendant flow (mirrors talkops_core::attendant::Node). */
export type FlowNode =
	| { type: 'play'; id: string; clip_id: string | null; next: FlowNode | null }
	| {
			type: 'menu';
			id: string;
			clip_id: string | null;
			timeout_secs: number;
			max_tries: number;
			direct_dial: boolean;
			options: { digit: string; next: FlowNode | null }[];
			timeout: FlowNode | null;
	  }
	| {
			type: 'ring';
			id: string;
			extensions: string[];
			strategy: 'simultaneous' | 'sequential';
			ring_secs: number;
			next: FlowNode | null;
	  }
	| {
			type: 'schedule';
			id: string;
			time_condition_id: string | null;
			open: FlowNode | null;
			closed: FlowNode | null;
	  }
	| {
			type: 'voicemail';
			id: string;
			recipients: string[];
			clip_id: string | null;
			max_message_secs: number;
	  }
	| { type: 'park'; id: string }
	| {
			type: 'transfer';
			id: string;
			destination_type: DestinationType;
			destination_id: string | null;
	  }
	| { type: 'goto'; id: string; target: string }
	| { type: 'hangup'; id: string };

export type FlowNodeType = FlowNode['type'];

export interface Attendant {
	id: string;
	number: string | null;
	name: string;
	language: 'de' | 'en' | null;
	flow: FlowNode;
}

export interface Queue {
	id: string;
	number: string | null;
	name: string;
	strategy: string;
	max_wait_secs: number;
	agent_timeout_secs: number;
	wrap_up_secs: number;
	timeout_type: DestinationType;
	timeout_id: string | null;
	enabled: boolean;
	members: string[];
	greeting_clip_id: string | null;
	moh_clip_id: string | null;
	max_callers: number;
	overflow_type: DestinationType;
	overflow_id: string | null;
	time_condition_id: string | null;
	closed_type: DestinationType;
	closed_id: string | null;
	voicemail_recipients: string[];
	voicemail_clip_id: string | null;
}

export interface HolidayCalendar {
	regions: [string, string][];
	holidays: { date: string; name: string }[];
}

/** Uploads a file with multipart/form-data (firmware). */
export async function upload<T>(path: string, form: FormData): Promise<T> {
	const res = await fetch(`/api/v1${path}`, {
		method: 'POST',
		headers: { 'X-Requested-With': 'TalkOps', 'X-CSRF-Token': csrfToken },
		body: form,
		credentials: 'same-origin'
	});
	const data = await res.json().catch(() => ({}));
	if (!res.ok)
		throw new ApiError(res.status, data.error ?? 'error', data.message ?? res.statusText);
	return data as T;
}

/** Fetches a plain-text API resource (phone configuration preview). */
export async function getText(path: string): Promise<string> {
	const res = await fetch(`/api/v1${path}`, {
		headers: { 'X-Requested-With': 'TalkOps' },
		credentials: 'same-origin'
	});
	if (!res.ok) {
		const data = await res.json().catch(() => ({}));
		throw new ApiError(res.status, data.error ?? 'error', data.message ?? res.statusText);
	}
	return res.text();
}

export function fetchStatus(): Promise<SystemStatus> {
	return api.get<SystemStatus>('/status');
}

export interface DoorButton {
	number: string;
	type: DestinationType;
	id: string | null;
}

/** Manufacturer of a door station (ADR 0026). */
export type DoorVendor = 'dahua';

export interface DoorStation {
	id: string;
	name: string;
	vendor: DoorVendor;
	extension_id: string;
	host: string;
	port: number;
	username: string;
	has_password: boolean;
	doors: number;
	destination_type: DestinationType;
	destination_id: string | null;
	buttons: DoorButton[];
	events_enabled: boolean;
	snapshots: boolean;
	has_webhook: boolean;
	has_api_token: boolean;
	/** A DTMF unlock code is stored for door 1 / door 2 (never returned). */
	has_unlock_code_1: boolean;
	has_unlock_code_2: boolean;
	/** External numbers (E.164) that may use the unlock codes; admins only. */
	dtmf_allowed_numbers: string[];
	online: boolean;
	model: string;
	last_seen: string | null;
	enabled: boolean;
}

export type DoorEventKind =
	| 'ring'
	| 'open_command'
	| 'opened'
	| 'door_open'
	| 'door_closed'
	| 'unlock_failed'
	| 'alarm'
	| 'online'
	| 'offline';

export interface DoorEvent {
	id: string;
	door_station_id: string;
	kind: DoorEventKind;
	detail: Record<string, unknown>;
	has_snapshot: boolean;
	created_at: string;
}

// --- transcription jobs -------------------------------------------------------

export type EngineChoice =
	| { kind: 'local'; model: 'base' | 'fast' | 'accurate' | 'best' | 'german' }
	| { kind: 'cloud'; provider_id: string; model: string };

export interface TranscribeOption {
	choice: EngineChoice;
	/** Cloud connection name; empty for local models. */
	provider: string;
	eta_secs: number;
	/** The estimate comes from measured runs on this server. */
	measured: boolean;
	cost_usd: number | null;
	/** The engine of the automatic first pass. */
	default: boolean;
}

export interface TranscribeOptions {
	audio_secs: number;
	queue_secs: number;
	current_engine: string | null;
	options: TranscribeOption[];
}

export type JobState = 'queued' | 'running' | 'done' | 'failed' | 'paused' | 'cancelled';

export interface TranscriptionJob {
	id: string;
	status: JobState;
	priority: number;
	progress: number | null;
	pass: number;
	engine: EngineChoice | null;
	engine_key: string;
	recording_id: string | null;
	voicemail_id: string | null;
	audio_secs: number;
	caller_number: string | null;
	caller_name: string | null;
	destination: string | null;
	/** Direction of the call; in the history also for voicemails. */
	direction: Call['direction'] | null;
	/** History only: parties resolved through the phone book. */
	caller?: CallParty | null;
	called?: CallParty | null;
	via_number?: string | null;
	call_started_at: string | null;
	requested_by: string | null;
	attempts: number;
	last_error: string | null;
	control: 'pause' | 'stop' | null;
	created_at: string;
	started_at: string | null;
	run_started_at: string | null;
	finished_at: string | null;
	eta_secs: number | null;
	resumable: boolean;
}

export interface WorkerStatus {
	worker_id: string;
	cpu_percent: number;
	cpus: number;
	load1: number;
	rss_bytes: number;
	mem_used_bytes: number;
	mem_total_bytes: number;
	job_id: string | null;
	updated_at: string;
}

export interface SystemLoad {
	cpu_percent: number | null;
	cpus: number;
	load1: number;
	load5: number;
	load15: number;
	mem_used_bytes: number;
	mem_total_bytes: number;
	uptime_secs: number;
}

export interface PhoneProvisioning {
	account_slots?: number | null;
	parameters?: Record<string, string>;
	language?: string;
	time_zone?: string;
	ntp_server?: string;
	vendor_settings?: Record<string, string>;
	defer_unassigned_keys?: boolean;
	endpoint?: string;
	sip_host?: string;
	firmware_profile?: string;
	template_id?: string | null;
	keys?: ProvisioningKey[];
	softkeys?: Record<string, string[]>;
}
export interface ProvisioningKey extends LineKey {
	native_function?: string;
	group: string;
	history_type: number;
	phonebook: number;
}
export interface PhoneParameter {
	key: string;
	parameter: string;
	description: string;
	artifact: string;
	category: string;
	kind: string;
	managed: boolean;
	copyable: boolean;
	default: string | null;
}
export interface KeyCapabilities {
	programmable: number[];
	expansion_modules: number;
	expansion_keys: number;
	softkey_states: Record<string, string[]>;
	programmable_functions: KeyType[];
	line_functions: KeyType[];
	firmware_profiles: string[];
}
export interface ProvisioningTemplate {
	id: string;
	name: string;
	vendor: string;
	model: string;
	active_revision: number | null;
}
export interface ProvisioningRevision {
	revision: number;
	files: Record<string, string>;
	created_at: string;
}
export interface ProvisioningRequest {
	mac: string | null;
	filename: string;
	scheme: string;
	status: number;
	created_at: string;
}
