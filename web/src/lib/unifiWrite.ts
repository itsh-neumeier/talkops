// Changes to the UniFi voice SSID (ADR 0042) and to the roaming threshold
// of Wi-Fi handsets (ADR 0043): API types and pure helpers for the diff
// dialog and the change history.

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

/** Phone setting of a handset change (ADR 0043). */
export const HANDSET_FIELD = 'wifi_roaming_threshold';

/** A field of a change row: an SSID field or the handset setting. */
export type ChangeField = FieldKey | typeof HANDSET_FIELD;

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

/** Band check result for one Wi-Fi handset. */
export interface HandsetBand {
	phone_id: string;
	name: string;
	state: 'ok' | 'blocked' | 'unknown';
	band: number | null;
}

/** Proposed roaming threshold of the survey handset. */
export interface HandsetProposal {
	phone_id: string;
	phone_name: string;
	model: string;
	setting: typeof HANDSET_FIELD;
	/** Own value of the phone. */
	current: Slot;
	/** Value the handset has now (own, tenant or factory value). */
	effective: string;
	proposed: string;
	recommendation: string;
	write: { allowed: boolean; reason: 'permission' | 'survey_active' | 'change_in_progress' | null };
}

export interface Proposal {
	wifi_id: string;
	wifi_name: string;
	changes: ProposalChange[];
	hints: ProposalHint[];
	base_hash: string;
	not_writable: 'ssid_type' | 'ssid_origin' | 'secret_not_readable' | null;
	write: WriteState;
	/** Present if a proposed change removes a band. */
	band_check: HandsetBand[] | null;
	handset: HandsetProposal | null;
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
	target: 'ssid' | 'handset';
	site_id: string | null;
	wifi_id: string | null;
	wifi_name: string;
	phone_id: string | null;
	phone_name: string;
	kind: 'apply' | 'rollback';
	rollback_of: string | null;
	survey_id: string | null;
	state: 'applying' | 'waiting' | 'applied' | 'failed' | 'rolled_back';
	fields: ChangeField[];
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
	field: ChangeField;
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
export function valueText(field: ChangeField, value: unknown, l: ValueLabels): string {
	if (value === null || value === undefined) return l.off;
	if (field === HANDSET_FIELD) return `${value} dBm`;
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

export function slotText(
	field: ChangeField,
	slot: Slot | null | undefined,
	l: ValueLabels
): string {
	if (!slot || isAbsent(slot)) return l.absent;
	return valueText(field, slot.value, l);
}

/** Request body of an apply for the selected rows. */
export function applyBody(
	p: Proposal,
	selected: ReadonlySet<FieldKey>,
	surveyId: string | null,
	confirmedHandsets: readonly string[] = []
): {
	survey_id: string | null;
	wifi_id: string;
	changes: { field: FieldKey; value: unknown }[];
	base_hash: string;
	confirm: true;
	confirm_unknown_handsets: string[];
} {
	return {
		survey_id: surveyId,
		wifi_id: p.wifi_id,
		changes: p.changes
			.filter((c) => selected.has(c.field))
			.map((c) => ({ field: c.field, value: c.proposed })),
		base_hash: p.base_hash,
		confirm: true,
		confirm_unknown_handsets: [...confirmedHandsets]
	};
}

/** Request body of a handset change (ADR 0043). */
export function handsetBody(h: HandsetProposal, surveyId: string | null) {
	return {
		survey_id: surveyId,
		phone_id: h.phone_id,
		value: h.proposed,
		base_value: 'value' in h.current ? (h.current.value as string) : null,
		confirm: true as const
	};
}

/**
 * Band protection of the selected rows: `blocked` if a handset uses a band
 * that the change removes; `unconfirmed` lists handsets that UniFi does not
 * report and that the user has not confirmed yet.
 */
export function bandState(
	p: Proposal,
	selected: ReadonlySet<FieldKey>,
	confirmed: ReadonlySet<string>
): { blocked: boolean; unconfirmed: HandsetBand[] } {
	if (!selected.has('broadcastingFrequenciesGHz') || !p.band_check) {
		return { blocked: false, unconfirmed: [] };
	}
	return {
		blocked: p.band_check.some((h) => h.state === 'blocked'),
		unconfirmed: p.band_check.filter((h) => h.state === 'unknown' && !confirmed.has(h.phone_id))
	};
}

/** Rows that are selected when the dialog opens. */
export function initialSelection(p: Proposal): Set<FieldKey> {
	return new Set(p.changes.filter((c) => c.selected).map((c) => c.field));
}

/** Warning IDs of the selected SSID rows; the client disconnect warning is first. */
export function warnings(p: Proposal, selected: ReadonlySet<FieldKey>): string[] {
	if (!selected.size) return [];
	const list = ['disconnect'];
	for (const c of p.changes) {
		if (c.warning && selected.has(c.field) && !list.includes(c.warning)) list.push(c.warning);
	}
	return list;
}

/** A change row that the user can roll back. */
export function canRollBack(c: WifiChange): boolean {
	return (
		c.kind === 'apply' &&
		!!c.before &&
		!!c.after &&
		(c.state === 'applied' || c.state === 'failed' || c.state === 'waiting')
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
	'verify_mismatch',
	'handset_band',
	'handset_unknown',
	'handset_changed',
	'not_wifi_handset'
] as const;

/** Translation key for an API error, or null for other errors. */
export function writeErrorKey(err: unknown): string | null {
	const code = (err as { code?: unknown } | null)?.code;
	return typeof code === 'string' && (ERROR_CODES as readonly string[]).includes(code)
		? `unifiWrite.err.${code}`
		: null;
}
