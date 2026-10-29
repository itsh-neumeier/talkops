import type { PhoneModel } from './api.ts';

// Catalog vendor ids are lowercase; show each manufacturer as it writes its name.
const NAMES: Record<string, string> = {
	'2n': '2N',
	aastra: 'Aastra',
	acrobits: 'Acrobits',
	algo: 'Algo',
	atcom: 'ATCOM',
	avaya: 'Avaya',
	cisco: 'Cisco',
	digium: 'Digium',
	escene: 'Escene',
	fanvil: 'Fanvil',
	flyingvoice: 'Flyingvoice',
	grandstream: 'Grandstream',
	groundwire: 'Groundwire',
	htek: 'Htek',
	linksys: 'Linksys',
	linphone: 'Linphone',
	mitel: 'Mitel',
	obihai: 'Obihai',
	panasonic: 'Panasonic',
	poly: 'Poly',
	polycom: 'Polycom',
	sangoma: 'Sangoma',
	sipnetic: 'Sipnetic',
	snom: 'Snom',
	swissvoice: 'Swissvoice',
	telekonnectors: 'Telekonnectors',
	vtech: 'VTech',
	yealink: 'Yealink',
	yeastar: 'Yeastar',
	zoiper: 'Zoiper'
};

export function vendorName(vendor: string | undefined): string {
	if (!vendor) return '';
	return NAMES[vendor.toLowerCase()] ?? vendor.charAt(0).toUpperCase() + vendor.slice(1);
}

/** "Yealink SIP-T41P"; a model name that already starts with the vendor is kept as is. */
export function modelLabel(model: Pick<PhoneModel, 'vendor' | 'name'>): string {
	const vendor = vendorName(model.vendor);
	if (!vendor || model.name.toLowerCase().startsWith(vendor.toLowerCase() + ' ')) return model.name;
	return `${vendor} ${model.name}`;
}

/** Models grouped by manufacturer name, then by model name. */
export function sortModels<T extends Pick<PhoneModel, 'vendor' | 'name'>>(models: T[]): T[] {
	return [...models].sort(
		(a, b) =>
			vendorName(a.vendor).localeCompare(vendorName(b.vendor)) ||
			a.name.localeCompare(b.name, undefined, { numeric: true })
	);
}
