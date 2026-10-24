// Network data of provisioned phones from the Ubiquiti Networks API
// (`GET /api/v1/phones/network`). Read only; off until an administrator
// switches it on in the connection settings.
import { rssiLevel } from './wifiSurvey.ts';

export interface NetClient {
	mac: string;
	wired: boolean;
	ip: string | null;
	network: string | null;
	vlan: number | null;
	ap_mac: string | null;
	essid: string | null;
	/** `ng` (2.4 GHz), `na` (5 GHz), `6e` (6 GHz). */
	radio: string | null;
	channel: number | null;
	/** Signal in dBm. */
	rssi: number | null;
	tx_rate_kbps: number | null;
	rx_rate_kbps: number | null;
	sw_mac: string | null;
	sw_port: number | null;
	wired_rate_mbps: number | null;
	/** UniFi experience value, 0 to 100. */
	satisfaction: number | null;
	uptime_secs: number | null;
	tx_bytes: number | null;
	rx_bytes: number | null;
}

export interface PhoneNetwork {
	phone_id: string;
	client: NetClient;
	ap_name: string | null;
	switch_name: string | null;
}

export type PhoneNetworkState = 'not_configured' | 'disabled' | 'ok' | 'error';

export interface PhoneNetworkView {
	state: PhoneNetworkState;
	error: string | null;
	read_at: string | null;
	phones: PhoneNetwork[];
}

/** Refresh interval of the network data on an open page. */
export const REFRESH_MS = 30_000;

/** The network data by phone ID. */
export function byPhone(view: PhoneNetworkView | null): Map<string, PhoneNetwork> {
	return new Map((view?.phones ?? []).map((p) => [p.phone_id, p]));
}

/** Signal bars (1 to 4) of an RSSI value; 0 without a value. */
export function signalBars(rssi: number | null | undefined): number {
	if (rssi == null) return 0;
	if (rssi >= -60) return 4;
	if (rssi >= -67) return 3;
	if (rssi >= -75) return 2;
	return 1;
}

/** Color class of the signal, from the voice thresholds of the Wi-Fi survey. */
export function signalClass(rssi: number | null | undefined): string {
	switch (rssiLevel(rssi)) {
		case 'clean':
			return 'text-emerald-600 dark:text-emerald-400';
		case 'audible':
			return 'text-amber-600 dark:text-amber-400';
		case 'bad':
			return 'text-red-600 dark:text-red-400';
		default:
			return 'text-slate-400';
	}
}

/** Link speed: `1 Gbit/s`, `100 Mbit/s`. */
export function linkSpeed(mbps: number | null | undefined): string {
	if (mbps == null) return '—';
	return mbps >= 1000 && mbps % 1000 === 0 ? `${mbps / 1000} Gbit/s` : `${mbps} Mbit/s`;
}

/** Byte count with a binary unit: `1.5 MB`. */
export function formatBytes(bytes: number | null | undefined): string {
	if (bytes == null) return '—';
	const units = ['B', 'KB', 'MB', 'GB', 'TB'];
	let value = bytes;
	let unit = 0;
	while (value >= 1024 && unit < units.length - 1) {
		value /= 1024;
		unit += 1;
	}
	return `${unit === 0 ? value : value.toFixed(1)} ${units[unit]}`;
}

/** Connection time: `3 d 4 h`, `2 h 5 min`, `45 min`. */
export function formatUptime(secs: number | null | undefined): string {
	if (secs == null) return '—';
	const d = Math.floor(secs / 86_400);
	const h = Math.floor((secs % 86_400) / 3_600);
	const m = Math.floor((secs % 3_600) / 60);
	if (d > 0) return `${d} d ${h} h`;
	if (h > 0) return `${h} h ${m} min`;
	return `${m} min`;
}
