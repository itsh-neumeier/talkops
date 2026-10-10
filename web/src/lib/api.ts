import { requestFinished, requestStarted } from './net.svelte.ts';
// Typed client for the TalkOps REST API (same origin, cookie session).

export class ApiError extends Error {
	constructor(
		public status: number,
		public code: string,
		message: string
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
	const headers: Record<string, string> = { 'X-Requested-With': 'TalkOps' };
	if (csrfToken) headers['X-CSRF-Token'] = csrfToken;
	if (body !== undefined) headers['Content-Type'] = 'application/json';
	const res = await fetch(`/api/v1${path}`, {
		method,
		headers,
		body: body === undefined ? undefined : JSON.stringify(body),
		credentials: 'same-origin'
	});
	if (res.status === 204) return undefined as T;
	const data = await res.json().catch(() => ({}));
	if (!res.ok) {
		if (res.status === 401 && !path.startsWith('/auth/login')) onUnauthorized();
		throw new ApiError(res.status, data.error ?? 'error', data.message ?? res.statusText);
	}
	return data as T;
}

export const api = {
	get: <T>(path: string) => request<T>('GET', path),
	post: <T>(path: string, body?: unknown) => request<T>('POST', path, body ?? {}),
	put: <T>(path: string, body: unknown) => request<T>('PUT', path, body),
	del: (path: string) => request<void>('DELETE', path)
};

// --- types (mirror the Rust API) -------------------------------------------

export type Role = 'admin' | 'operator' | 'user';

export interface User {
	id: string;
	username: string;
	display_name: string;
	email: string | null;
	role: Role;
	enabled: boolean;
	auth_source: string;
	last_login_at: string | null;
	totp_enabled: boolean;
}

export interface Me {
	user: User;
	csrf_token: string;
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
}

export type DeviceKind =
	'desk' | 'dect' | 'wifi' | 'softphone' | 'mobile' | 'door' | 'other' | 'browser';

export interface Device {
	id: string;
	extension_id: string;
	name: string;
	kind: DeviceKind;
	sip_username: string;
	phone_id: string | null;
	account_index: number | null;
	/** Label on the provisioned phone; empty = extension number. */
	phone_label: string;
	/** Caller name sent by the provisioned phone; empty = display name. */
	phone_display_name: string;
	enabled: boolean;
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
	/** Engine of the quick first pass. */
	transcription_quality: TranscriptionEngine;
	/** Engine of the more accurate second pass; '' = none. */
	transcription_refine: TranscriptionEngine | '';
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
	recording_id: string | null;
}

export type StatsRange = '1h' | '1d' | '1w' | '1m';

export interface CallStats {
	range: StatsRange;
	buckets: { start: string; inbound: number; outbound: number; internal: number; missed: number }[];
	end: string;
	total: number;
	inbound: number;
	outbound: number;
	internal: number;
	missed: number;
	answer_rate: number | null;
	avg_talk_secs: number;
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
	username: string | null;
	action: string;
	entity_type: string;
	entity_id: string | null;
	details: Record<string, unknown>;
	ip: string | null;
	created_at: string;
}

export type KeyType = 'none' | 'line' | 'blf' | 'speed_dial';

export interface LineKey {
	key: number;
	type: KeyType;
	value: string;
	label: string;
	account: number;
}

export interface Phone {
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
	key: string;
	group: string;
	kind: 'bool' | 'choice';
	values: string[];
	/** The phone's factory value, if documented. */
	default: string | null;
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
	accounts: PhoneAccount[];
}

export interface PhoneModel {
	id: string;
	name: string;
	vendor: string;
	family: 'desk' | 'dect' | 'conference' | 'wifi';
	accounts: number;
	line_keys: number;
	/** Line keys can be speed dials (not on Wi-Fi handsets). */
	speed_dial_keys: boolean;
	video: boolean;
	/** Largest custom ringtone in KiB; 0 = not supported. */
	ringtone_max_kb: number;
	wallpaper: boolean;
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

export type TranscriptionEngine = 'fast' | 'accurate' | 'best' | 'german' | 'api';

/** OpenAI-compatible transcription API (key never returned). */
export interface TranscriptionApi {
	url: string;
	model: string;
	has_key: boolean;
}

export interface SmtpSettings {
	host: string;
	port: number;
	security: 'starttls' | 'tls' | 'none';
	username: string;
	has_password: boolean;
	from: string;
}

export type DestinationType =
	'none' | 'extension' | 'voicemail' | 'ring_group' | 'time_condition' | 'ivr' | 'queue';

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

export interface DoorStation {
	id: string;
	name: string;
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
