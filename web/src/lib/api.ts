export interface Component {
	ok: boolean;
	detail?: string;
}

export interface SystemStatus {
	version: string;
	database: Component;
	freeswitch: Component;
}

export async function fetchStatus(): Promise<SystemStatus> {
	const res = await fetch('/api/v1/status');
	if (!res.ok) throw new Error(`HTTP ${res.status}`);
	return res.json();
}
