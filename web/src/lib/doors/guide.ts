// Setup guides for door stations, as data. Each vendor has one guide per
// device type; sections reference i18n keys so that both languages stay
// complete. Device field names (e.g. `Sip Register Pwd`) are shown as they
// appear on the device and are not translated.
import type { MessageKey } from '../i18n/index.svelte.ts';

/** A translated text, or a literal label from the device's own interface. */
export type GuideText = MessageKey | { raw: string };

export interface GuideTable {
	caption?: MessageKey;
	columns: MessageKey[];
	rows: GuideText[][];
}

export interface GuideSection {
	id: string;
	title: MessageKey;
	intro?: MessageKey;
	/** Numbered steps. */
	steps?: MessageKey[];
	table?: GuideTable;
	notes?: MessageKey[];
	/** Information that is not confirmed with real devices. */
	unconfirmed?: MessageKey[];
}

export interface DeviceGuide {
	/** Part of the URL hash, e.g. `vto` in `#guide/dahua/vto`. */
	id: string;
	label: MessageKey;
	summary: MessageKey;
	sections: GuideSection[];
}

export interface VendorGuide {
	id: string;
	/** Brand name; not translated. */
	name: string;
	devices: DeviceGuide[];
}

export const raw = (text: string): GuideText => ({ raw: text });

/** All i18n keys of a list of sections (for completeness tests). */
export function sectionKeys(sections: GuideSection[]): MessageKey[] {
	const keys: MessageKey[] = [];
	const add = (t: GuideText | undefined) => {
		if (t !== undefined && typeof t === 'string') keys.push(t);
	};
	for (const s of sections) {
		add(s.title);
		add(s.intro);
		s.steps?.forEach(add);
		s.notes?.forEach(add);
		s.unconfirmed?.forEach(add);
		add(s.table?.caption);
		s.table?.columns.forEach(add);
		s.table?.rows.forEach((r) => r.forEach(add));
	}
	return keys;
}

/** The guide selection in the URL hash: `#guide/<vendor>/<device>`. */
export interface GuideLocation {
	vendor: string;
	device: string;
}

/**
 * Reads `#guide`, `#guide/<vendor>` or `#guide/<vendor>/<device>`. Unknown
 * parts fall back to the first vendor or device. Returns `null` for other
 * hashes (the station list).
 */
export function parseGuideHash(hash: string, vendors: VendorGuide[]): GuideLocation | null {
	const parts = hash.replace(/^#/, '').split('/');
	if (parts[0] !== 'guide' || vendors.length === 0) return null;
	const vendor = vendors.find((v) => v.id === parts[1]) ?? vendors[0];
	const device = vendor.devices.find((d) => d.id === parts[2]) ?? vendor.devices[0];
	return { vendor: vendor.id, device: device?.id ?? '' };
}

export function guideHash(loc: GuideLocation): string {
	return `#guide/${loc.vendor}/${loc.device}`;
}
