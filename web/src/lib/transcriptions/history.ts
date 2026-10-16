// Filter and sort state of the transcription history, kept in the address
// bar. The history has the jobs of the last 7 days (at most 1000), so the
// browser filters and sorts the full list.
import { digitsMatch, parseOrder, pickList, queryString, type Order } from '../list/sort.ts';

export type HistoryStatus = 'done' | 'failed' | 'cancelled';
export type Direction = 'inbound' | 'outbound' | 'internal';
export type SortKey = 'time' | 'duration' | 'status';

/** Final states of a job (the history has no running or queued jobs). */
export const HISTORY_STATUSES: HistoryStatus[] = ['done', 'failed', 'cancelled'];
export const DIRECTIONS: Direction[] = ['inbound', 'outbound', 'internal'];
export const SORT_KEYS: SortKey[] = ['time', 'duration', 'status'];
/** Days in the history (same as the server). */
export const HISTORY_DAYS = 7;
/** Maximum number of jobs in the history (same as the server). */
export const HISTORY_LIMIT = 1000;

export interface HistoryFilters {
	status: HistoryStatus[];
	/** Engine keys (`local:fast`, `cloud:<id>:<model>`). */
	engine: string[];
	direction: Direction[];
	/** Local date `YYYY-MM-DD`, inclusive. */
	since: string;
	/** Local date `YYYY-MM-DD`, inclusive. */
	until: string;
	search: string;
	sort: SortKey;
	order: Order;
}

interface Party {
	name: string;
	number: string;
}

/** The job fields that the filters and the sort use. */
export interface HistoryRow {
	status: string;
	engine_key: string;
	direction: Direction | null;
	caller_number: string | null;
	caller_name: string | null;
	destination: string | null;
	caller?: Party | null;
	called?: Party | null;
	via_number?: string | null;
	requested_by: string | null;
	created_at: string;
	started_at: string | null;
	finished_at: string | null;
}

export function emptyFilters(): HistoryFilters {
	return {
		status: [],
		engine: [],
		direction: [],
		since: '',
		until: '',
		search: '',
		sort: 'time',
		order: 'desc'
	};
}

const DATE = /^\d{4}-\d{2}-\d{2}$/;

/** Reads the filters from the query string of the page address. */
export function parseFilters(params: { get(name: string): string | null }): HistoryFilters {
	const f = emptyFilters();
	f.status = pickList(HISTORY_STATUSES, params.get('status'));
	f.engine = (params.get('engine') ?? '')
		.split(',')
		.filter((v, i, all) => v !== '' && all.indexOf(v) === i);
	f.direction = pickList(DIRECTIONS, params.get('direction'));
	const since = params.get('since') ?? '';
	const until = params.get('until') ?? '';
	f.since = DATE.test(since) ? since : '';
	f.until = DATE.test(until) ? until : '';
	f.search = params.get('search') ?? '';
	const sort = params.get('sort') ?? '';
	f.sort = (SORT_KEYS as string[]).includes(sort) ? (sort as SortKey) : 'time';
	f.order = parseOrder(params.get('order'), 'desc');
	return f;
}

/** The query string of the page address (history tab); defaults are left out. */
export function pageQuery(f: HistoryFilters): string {
	const p = new URLSearchParams();
	p.set('tab', 'history');
	if (f.status.length) p.set('status', f.status.join(','));
	if (f.engine.length) p.set('engine', f.engine.join(','));
	if (f.direction.length) p.set('direction', f.direction.join(','));
	if (f.since) p.set('since', f.since);
	if (f.until) p.set('until', f.until);
	if (f.search.trim()) p.set('search', f.search.trim());
	if (f.sort !== 'time') p.set('sort', f.sort);
	if (f.order !== 'desc') p.set('order', f.order);
	return queryString(p);
}

/** Active filters of each column (for the filter mark). */
export function activeFilters(f: HistoryFilters): Record<string, boolean> {
	return {
		time: !!(f.since || f.until),
		direction: f.direction.length > 0,
		engine: f.engine.length > 0,
		status: f.status.length > 0
	};
}

export function anyFilter(f: HistoryFilters): boolean {
	return Object.values(activeFilters(f)).some(Boolean) || !!f.search.trim();
}

/** Time shown for a job: the end, else the creation. */
export function jobTime(j: Pick<HistoryRow, 'finished_at' | 'created_at'>): string {
	return j.finished_at ?? j.created_at;
}

/** Processing time in seconds; null when the job did not run. */
export function runSecs(j: Pick<HistoryRow, 'started_at' | 'finished_at'>): number | null {
	if (!j.started_at || !j.finished_at) return null;
	return Math.max(0, Math.round((Date.parse(j.finished_at) - Date.parse(j.started_at)) / 1000));
}

/** Local date `YYYY-MM-DD` of an ISO time. */
export function localDate(iso: string): string {
	const d = new Date(iso);
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** True if a party of the job contains the text (name or number). */
export function matchesParties(j: HistoryRow, search: string): boolean {
	const q = search.trim().toLowerCase();
	if (!q) return true;
	const numbers = [
		j.caller?.number,
		j.called?.number,
		j.via_number,
		j.caller_number,
		j.destination
	].filter((v): v is string => !!v);
	const names = [j.caller?.name, j.called?.name, j.caller_name, j.requested_by].filter(
		(v): v is string => !!v
	);
	if ([...names, ...numbers].some((v) => v.toLowerCase().includes(q))) return true;
	return digitsMatch(q, numbers);
}

/** True if the job matches all filters. */
export function matches(j: HistoryRow, f: HistoryFilters): boolean {
	if (f.status.length && !(f.status as string[]).includes(j.status)) return false;
	if (f.engine.length && !f.engine.includes(j.engine_key)) return false;
	if (f.direction.length && !(j.direction && f.direction.includes(j.direction))) return false;
	if (f.since || f.until) {
		const day = localDate(jobTime(j));
		if (f.since && day < f.since) return false;
		if (f.until && day > f.until) return false;
	}
	return matchesParties(j, f.search);
}

const STATUS_RANK: Record<string, number> = { done: 0, failed: 1, cancelled: 2 };

/**
 * Sorts a copy of the list. Jobs without a processing time come last in
 * both directions. Equal values are sorted by time, newest first.
 */
export function sortJobs<T extends HistoryRow>(
	list: readonly T[],
	f: Pick<HistoryFilters, 'sort' | 'order'>
): T[] {
	const dir = f.order === 'asc' ? 1 : -1;
	const byTime = (a: T, b: T) => Date.parse(jobTime(a)) - Date.parse(jobTime(b));
	return [...list].sort((a, b) => {
		if (f.sort === 'time') return dir * byTime(a, b);
		let r = 0;
		if (f.sort === 'duration') {
			const ra = runSecs(a);
			const rb = runSecs(b);
			if ((ra === null) !== (rb === null)) return ra === null ? 1 : -1;
			r = dir * ((ra ?? 0) - (rb ?? 0));
		} else {
			r = dir * ((STATUS_RANK[a.status] ?? 9) - (STATUS_RANK[b.status] ?? 9));
		}
		return r || -byTime(a, b);
	});
}
