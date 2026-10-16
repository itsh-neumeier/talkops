// Filter and sort state of the call list, kept in the address bar.
import { parseOrder, pickList, queryString, type Order } from '../list/sort.ts';

export type { Order };

export type Direction = 'inbound' | 'outbound' | 'internal';
export type Result = 'answered' | 'missed' | 'busy';
export type RecordingFilter = '' | 'with' | 'transcript' | 'without';
export type SortKey = 'time' | 'duration';

export interface CallFilters {
	/** Local date `YYYY-MM-DD`, inclusive. */
	since: string;
	/** Local date `YYYY-MM-DD`, inclusive. */
	until: string;
	direction: Direction[];
	result: Result[];
	from: string;
	to: string;
	recording: RecordingFilter;
	search: string;
	sort: SortKey;
	order: Order;
}

export const DIRECTIONS: Direction[] = ['inbound', 'outbound', 'internal'];
export const RESULTS: Result[] = ['answered', 'missed', 'busy'];
export const RECORDING_FILTERS: RecordingFilter[] = ['with', 'transcript', 'without'];
/** Hangup causes shown as "busy or declined" (same list as the server). */
export const BUSY_CAUSES = ['USER_BUSY', 'CALL_REJECTED'];

export function emptyFilters(): CallFilters {
	return {
		since: '',
		until: '',
		direction: [],
		result: [],
		from: '',
		to: '',
		recording: '',
		search: '',
		sort: 'time',
		order: 'desc'
	};
}

const DATE = /^\d{4}-\d{2}-\d{2}$/;

/** Reads the filters from the query string of the page address. */
export function parseFilters(params: { get(name: string): string | null }): CallFilters {
	const f = emptyFilters();
	const since = params.get('since') ?? '';
	const until = params.get('until') ?? '';
	f.since = DATE.test(since) ? since : '';
	f.until = DATE.test(until) ? until : '';
	f.direction = pickList(DIRECTIONS, params.get('direction'));
	f.result = pickList(RESULTS, params.get('result'));
	f.from = params.get('from') ?? '';
	f.to = params.get('to') ?? '';
	const rec = params.get('recording') ?? '';
	f.recording = (RECORDING_FILTERS as string[]).includes(rec) ? (rec as RecordingFilter) : '';
	f.search = params.get('search') ?? '';
	f.sort = params.get('sort') === 'duration' ? 'duration' : 'time';
	f.order = parseOrder(params.get('order'), 'desc');
	return f;
}

/** The query string of the page address; defaults are left out. */
export function pageQuery(f: CallFilters): string {
	const p = new URLSearchParams();
	if (f.since) p.set('since', f.since);
	if (f.until) p.set('until', f.until);
	if (f.direction.length) p.set('direction', f.direction.join(','));
	if (f.result.length) p.set('result', f.result.join(','));
	if (f.from.trim()) p.set('from', f.from.trim());
	if (f.to.trim()) p.set('to', f.to.trim());
	if (f.recording) p.set('recording', f.recording);
	if (f.search.trim()) p.set('search', f.search.trim());
	if (f.sort !== 'time') p.set('sort', f.sort);
	if (f.order !== 'desc') p.set('order', f.order);
	return queryString(p);
}

/** Start of a local day as an ISO time; `days` moves the date. */
export function localDayStart(date: string, days = 0): string {
	const [y, m, d] = date.split('-').map(Number);
	return new Date(y, m - 1, d + days).toISOString();
}

/** The local date `days` before today as `YYYY-MM-DD`. */
export function daysAgo(days: number, now = new Date()): string {
	const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - days);
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** The query string of the API request for one page of the list. */
export function apiQuery(f: CallFilters, offset: number, limit: number): string {
	const p = new URLSearchParams();
	if (f.since) p.set('since', localDayStart(f.since));
	// The end date is inclusive: up to the start of the next day.
	if (f.until) p.set('until', localDayStart(f.until, 1));
	if (f.direction.length) p.set('direction', f.direction.join(','));
	if (f.result.length) p.set('result', f.result.join(','));
	if (f.from.trim()) p.set('from', f.from.trim());
	if (f.to.trim()) p.set('to', f.to.trim());
	if (f.recording) p.set('recording', f.recording);
	if (f.search.trim()) p.set('search', f.search.trim());
	p.set('sort', f.sort);
	p.set('order', f.order);
	p.set('offset', String(offset));
	p.set('limit', String(limit));
	return `?${p}`;
}

/** Number of active filters of one column (for the filter mark). */
export function activeCount(f: CallFilters): Record<string, boolean> {
	return {
		time: !!(f.since || f.until),
		direction: f.direction.length > 0,
		from: !!f.from.trim(),
		to: !!f.to.trim(),
		result: f.result.length > 0,
		recording: !!f.recording
	};
}

/** Result of a call: answered, busy or declined, or missed. */
export function callResult(c: { answered_at: string | null; hangup_cause: string }): Result {
	if (c.answered_at) return 'answered';
	return BUSY_CAUSES.includes(c.hangup_cause) ? 'busy' : 'missed';
}
