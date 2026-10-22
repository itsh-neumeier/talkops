// Changes to the UniFi voice SSID (ADR 0042): API types and pure helpers
// for the diff dialog and the change history.

/** A field value, or a field that the SSID object does not contain. */
export type Slot = { value: unknown } | { absent: true };

export type FieldKey =
	| 'bssTransitionEnabled'
	| 'securityConfiguration.wpa3FastRoamingEnabled'
	| 'securityConfiguration.pmfMode'
	| 'handoffSuggestionsConfiguration'
	| 'broadcastingFrequenciesGHz'
	| 'uapsdEnabled'
	| 'multicastToUnicastConversionEnabled'
	| 'arpProxyEnabled'
	| 'dtimPeriodByFrequencyGHzOverride';

export const FIELD_KEYS: FieldKey[] = [
	'bssTransitionEnabled',
	'securityConfiguration.wpa3FastRoamingEnabled',
	'securityConfiguration.pmfMode',
	'handoffSuggestionsConfiguration',
	'broadcastingFrequenciesGHz',
	'uapsdEnabled',
	'multicastToUnicastConversionEnabled',
	'arpProxyEnabled',
	'dtimPeriodByFrequencyGHzOverride'
];

export interface ProposalChange {
	field: FieldKey;
	current: Slot;
	proposed: unknown;
	recommendation: string;
	selected: boolean;
	warning?: 'fast_roaming_compat' | 'band_2g_removed';
}

export interface ProposalHint {
	id: string;
	recommendation: string;
}

export type WriteReason =
	| 'write_disabled'
	| 'permission'
	| 'version_unsupported'
	| 'no_voice_ssid'
	| 'survey_active'
	| 'change_in_progress';

export interface WriteState {
	allowed: boolean;
	reason: WriteReason | null;
	version: string | null;
	min_version: string;
}

export interface Proposal {
	wifi_id: string;
	wifi_name: string;
	changes: ProposalChange[];
	hints: ProposalHint[];
	base_hash: string;
	not_writable: 'ssid_type' | 'ssid_origin' | 'secret_not_readable' | null;
	write: WriteState;
}

export interface WifiSummary {
	id: string;
	name: string;
	type: string;
	enabled: boolean;
	origin: string | null;
	security: string | null;
	frequencies: number[];
}

export interface WifiChange {
	id: string;
	site_id: string;
	wifi_id: string;
	wifi_name: string;
	kind: 'apply' | 'rollback';
	rollback_of: string | null;
	survey_id: string | null;
	state: 'applying' | 'applied' | 'failed' | 'rolled_back';
	fields: FieldKey[];
	before: Record<string, Slot> | null;
	after: Record<string, Slot> | null;
	network_version: string | null;
	error: string | null;
	created_by: string | null;
	created_by_name: string | null;
	created_at: string;
	finished_at: string | null;
}

export interface ForeignField {
	field: FieldKey;
	after: Slot;
	current: Slot;
	before: Slot | null;
}

/** Words that the caller translates. */
export interface ValueLabels {
	on: string;
	off: string;
	absent: string;
}

const isAbsent = (s: Slot): s is { absent: true } => 'absent' in s;

/** Display text of a field value. */
export function valueText(field: FieldKey, value: unknown, l: ValueLabels): string {
	if (value === null || value === undefined) return l.off;
	if (typeof value === 'boolean') return value ? l.on : l.off;
	if (field === 'broadcastingFrequenciesGHz' && Array.isArray(value)) {
		return value.map((f) => `${f} GHz`).join(' + ');
	}
	if (field === 'handoffSuggestionsConfiguration' && typeof value === 'object') {
		const v = value as Record<string, number | null>;
		const one = (n: number | null | undefined) => (n == null ? l.off : `${n} dBm`);
		return `5 GHz: ${one(v.band5GHzRssiThreshold)} · 6 GHz: ${one(v.band6GHzRssiThreshold)}`;
	}
	if (field === 'dtimPeriodByFrequencyGHzOverride' && typeof value === 'object') {
		const v = value as Record<string, number>;
		return ['2.4', '5', '6'].map((b) => `${b} GHz: ${v[b] ?? '—'}`).join(' · ');
	}
	if (typeof value === 'object') return JSON.stringify(value);
	return String(value);
}

export function slotText(field: FieldKey, slot: Slot | null | undefined, l: ValueLabels): string {
	if (!slot || isAbsent(slot)) return l.absent;
	return valueText(field, slot.value, l);
}

/** Request body of an apply for the selected rows. */
export function applyBody(
	p: Proposal,
	selected: ReadonlySet<FieldKey>,
	surveyId: string | null
): {
	survey_id: string | null;
	wifi_id: string;
	changes: { field: FieldKey; value: unknown }[];
	base_hash: string;
	confirm: true;
} {
	return {
		survey_id: surveyId,
		wifi_id: p.wifi_id,
		changes: p.changes
			.filter((c) => selected.has(c.field))
			.map((c) => ({ field: c.field, value: c.proposed })),
		base_hash: p.base_hash,
		confirm: true
	};
}

/** Rows that are selected when the dialog opens. */
export function initialSelection(p: Proposal): Set<FieldKey> {
	return new Set(p.changes.filter((c) => c.selected).map((c) => c.field));
}

/** Warning IDs of the selected rows; the client disconnect warning is always first. */
export function warnings(p: Proposal, selected: ReadonlySet<FieldKey>): string[] {
	const list = ['disconnect'];
	for (const c of p.changes) {
		if (c.warning && selected.has(c.field) && !list.includes(c.warning)) list.push(c.warning);
	}
	return list;
}

/** A change row that the user can roll back. */
export function canRollBack(c: WifiChange): boolean {
	return (
		c.kind === 'apply' && !!c.before && !!c.after && (c.state === 'applied' || c.state === 'failed')
	);
}

/** Error codes of the API with their own UI text (`unifiWrite.err.<code>`). */
export const ERROR_CODES = [
	'write_disabled',
	'permission',
	'version_unsupported',
	'no_voice_ssid',
	'survey_active',
	'change_in_progress',
	'ssid_changed',
	'ssid_not_voice',
	'ssid_type',
	'ssid_origin',
	'secret_not_readable',
	'not_revertible',
	'unifi_forbidden',
	'verify_mismatch'
] as const;

/** Translation key for an API error, or null for other errors. */
export function writeErrorKey(err: unknown): string | null {
	const code = (err as { code?: unknown } | null)?.code;
	return typeof code === 'string' && (ERROR_CODES as readonly string[]).includes(code)
		? `unifiWrite.err.${code}`
		: null;
}
