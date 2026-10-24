// Outbound dialing permissions (ADR 0045): profile types and the rule
// helpers of the profile editor. The API validates and decides; these
// functions only prepare input and display.
import { getCountries } from 'libphonenumber-js/max';

export type RuleKind = 'country' | 'prefix' | 'number';
export type RuleAction = 'allow' | 'deny';

export type DialingRule = {
	id?: string;
	kind: RuleKind;
	value: string;
	action: RuleAction;
	label: string;
};

export type DialingProfile = {
	id: string;
	name: string;
	description: string;
	base_mode: RuleAction;
	sip_addresses: boolean;
	is_default: boolean;
	rules: DialingRule[];
	extensions: number;
	updated_at: string;
};

export type PremiumRule = { prefix: string; label: string; source: string };

export type DialTestResult = {
	class: 'emergency' | 'service' | 'external' | 'invalid';
	number: string | null;
	country: string | null;
	profile: { id: string; name: string } | null;
	allowed: boolean;
	rule: DialingRule | null;
	reason: string | null;
};

/** The value as the API stores it, or null if the API would reject it. */
export function normalizeValue(kind: RuleKind, value: string): string | null {
	const compact = value.replace(/[\s\-/()]/g, '');
	if (kind === 'country') {
		const iso = compact.toUpperCase();
		return /^[A-Z]{2}$/.test(iso) ? iso : null;
	}
	if (kind === 'prefix') {
		const prefix = compact.endsWith('*') ? compact.slice(0, -1) : compact;
		return /^\+\d{1,15}$/.test(prefix) ? prefix : null;
	}
	return /^\+[1-9]\d{2,14}$/.test(compact) ? compact : null;
}

const rank: Record<RuleKind, number> = { number: 0, prefix: 1, country: 2 };

/** Rules in the order of their effect: numbers, longest prefixes, countries. */
export function sortRules<T extends DialingRule>(rules: T[]): T[] {
	return [...rules].sort(
		(a, b) =>
			rank[a.kind] - rank[b.kind] ||
			b.value.length - a.value.length ||
			a.value.localeCompare(b.value)
	);
}

/** Adds a rule; the same kind and value replaces the older rule. */
export function upsertRule(rules: DialingRule[], rule: DialingRule): DialingRule[] {
	return sortRules([
		...rules.filter((r) => !(r.kind === rule.kind && r.value === rule.value)),
		rule
	]);
}

/** The premium-rate blocks that the profile does not contain yet. */
export function missingPremium(rules: DialingRule[], premium: PremiumRule[]): DialingRule[] {
	return premium
		.filter((p) => !rules.some((r) => r.kind === 'prefix' && r.value === p.prefix))
		.map((p) => ({ kind: 'prefix', value: p.prefix, action: 'deny', label: p.label }));
}

export type ReasonParts = { kind: 'base' | 'sip' } | { kind: RuleKind; value: string } | null;

/** Splits a `dialing:…` block reason of a call record. */
export function reasonParts(reason: string): ReasonParts {
	if (reason === 'dialing:base') return { kind: 'base' };
	if (reason === 'dialing:sip') return { kind: 'sip' };
	const m = /^dialing:(country|prefix|number):(.+)$/.exec(reason);
	if (!m) return null;
	const kind = m[1] as RuleKind;
	return normalizeValue(kind, m[2]) === m[2] ? { kind, value: m[2] } : null;
}

/** ISO codes of all countries, by their name in `locale`. */
export function countryOptions(locale: string): { code: string; name: string }[] {
	const names = new Intl.DisplayNames([locale], { type: 'region' });
	return getCountries()
		.map((code) => ({ code: code as string, name: names.of(code) ?? code }))
		.sort((a, b) => a.name.localeCompare(b.name, locale));
}

/** The body of a create or update request. */
export function profileInput(
	p: Omit<DialingProfile, 'id' | 'is_default' | 'extensions' | 'updated_at'>
) {
	return {
		name: p.name.trim(),
		description: p.description.trim(),
		base_mode: p.base_mode,
		sip_addresses: p.sip_addresses,
		rules: p.rules.map(({ kind, value, action, label }) => ({ kind, value, action, label }))
	};
}
