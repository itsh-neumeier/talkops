// Shared helpers of the dashboard charts: round axis maxima, smooth lines,
// stacked series and the trend against the previous period. Plain SVG; no
// chart library.

export interface Point {
	x: number;
	y: number;
}

/**
 * A round axis with about `count` integer steps (1, 2, 5, 10, 20 …): the
 * grid labels are whole numbers. `max` is at least `min`.
 */
export function niceScale(values: number[], count = 4, min = 4): { max: number; step: number } {
	const top = Math.max(min, ...values);
	const rough = top / count;
	const base = 10 ** Math.floor(Math.log10(rough));
	const step = Math.max(1, [1, 2, 5, 10].map((m) => m * base).find((v) => v >= rough) ?? rough);
	return { max: Math.ceil(top / step) * step, step };
}

/**
 * A smooth line through the points (monotone cubic, Fritsch–Carlson): no
 * overshoot below zero or above a peak.
 */
export function smoothPath(points: Point[]): string {
	const n = points.length;
	if (n === 0) return '';
	const f = (v: number) => Math.round(v * 10) / 10;
	if (n === 1) return `M${f(points[0].x)},${f(points[0].y)}`;
	const dx = points.slice(1).map((p, i) => p.x - points[i].x);
	const slope = points.slice(1).map((p, i) => (p.y - points[i].y) / (dx[i] || 1));
	const m = points.map((_, i) => {
		if (i === 0) return slope[0];
		if (i === n - 1) return slope[n - 2];
		return slope[i - 1] * slope[i] <= 0 ? 0 : (slope[i - 1] + slope[i]) / 2;
	});
	for (let i = 0; i < n - 1; i++) {
		if (slope[i] === 0) {
			m[i] = 0;
			m[i + 1] = 0;
			continue;
		}
		const a = m[i] / slope[i];
		const b = m[i + 1] / slope[i];
		const h = a * a + b * b;
		if (h > 9) {
			const t = 3 / Math.sqrt(h);
			m[i] = t * a * slope[i];
			m[i + 1] = t * b * slope[i];
		}
	}
	let d = `M${f(points[0].x)},${f(points[0].y)}`;
	for (let i = 0; i < n - 1; i++) {
		const p = points[i];
		const q = points[i + 1];
		const h = dx[i] / 3;
		d += `C${f(p.x + h)},${f(p.y + m[i] * h)},${f(q.x - h)},${f(q.y - m[i + 1] * h)},${f(q.x)},${f(q.y)}`;
	}
	return d;
}

/** A closed area under a smooth line down to `base`. */
export function areaPath(points: Point[], base: number): string {
	if (points.length === 0) return '';
	const f = (v: number) => Math.round(v * 10) / 10;
	return `${smoothPath(points)}L${f(points.at(-1)!.x)},${f(base)}L${f(points[0].x)},${f(base)}Z`;
}

/** Running sums of stacked series: `out[k][i]` = series 0..k at slot i. */
export function stack(series: number[][]): number[][] {
	const out: number[][] = [];
	series.forEach((values, k) => {
		out.push(values.map((v, i) => v + (k > 0 ? out[k - 1][i] : 0)));
	});
	return out;
}

export type TrendDirection = 'up' | 'down' | 'flat';

export interface Trend {
	/** Change in percent, rounded; `null` without a base value. */
	percent: number | null;
	direction: TrendDirection;
}

/** Change of `current` against `previous`. */
export function trend(current: number | null, previous: number | null): Trend | null {
	if (current == null || previous == null) return null;
	const direction: TrendDirection =
		current > previous ? 'up' : current < previous ? 'down' : 'flat';
	if (previous === 0) return { percent: null, direction };
	return { percent: Math.round(((current - previous) / previous) * 100), direction };
}

/** Color class of a trend; `upIsGood` is false for missed calls. */
export function trendClass(t: Trend | null, upIsGood = true): string {
	if (!t || t.direction === 'flat') return 'text-slate-500 dark:text-slate-400';
	const good = (t.direction === 'up') === upIsGood;
	return good ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400';
}

/** Points of a small line chart of `values` in a `w`×`h` box. */
export function sparkPoints(values: number[], w: number, h: number, pad = 2): Point[] {
	if (values.length === 0) return [];
	const max = Math.max(1, ...values);
	const step = values.length > 1 ? (w - pad * 2) / (values.length - 1) : 0;
	return values.map((v, i) => ({ x: pad + i * step, y: h - pad - (v / max) * (h - pad * 2) }));
}

/** Stroke offset of a ring gauge for `percent` (0 to 100) of `circumference`. */
export function ringOffset(percent: number | null, circumference: number): number {
	const p = Math.max(0, Math.min(100, percent ?? 0));
	return circumference * (1 - p / 100);
}
