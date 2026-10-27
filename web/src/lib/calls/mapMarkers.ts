import type { MapArea } from '../api.ts';
import { bubbleGradient, CATEGORIES, CATEGORY_COLORS, type Category } from './map.ts';

export function markerKind(a: MapArea): 'area' | 'mobile' | 'unlocated' {
	return (
		a.marker_kind ??
		(a.kind === 'area'
			? 'area'
			: a.mobile > 0 && !a.service && !a.unresolved
				? 'mobile'
				: 'unlocated')
	);
}

export function markerName(a: MapArea, noLocation: string): string {
	return a.kind === 'area' ? a.name : `${a.country ?? ''} ${noLocation}`.trim();
}

export function escapeHtml(text: string): string {
	return text.replace(
		/[&<>"']/g,
		(c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!
	);
}

/** Data marks, not a physical device or caller location. All text is escaped. */
export function markerHtml(
	a: MapArea,
	shown: Category[],
	size: number,
	dark: boolean,
	name: string,
	decorated: boolean
): string {
	const kind = markerKind(a);
	const pie = `<span class="call-device-pie" style="width:${size}px;height:${size}px;background:${bubbleGradient(a.counts, shown, dark)}"></span>`;
	const keys =
		kind === 'area' ? '<span class="call-device-keys">' + '<b></b>'.repeat(9) + '</span>' : '';
	const stats = decorated
		? `<span class="call-marker-stats">${CATEGORIES.filter((c) => shown.includes(c))
				.map(
					(c) =>
						`<span><i style="background:${CATEGORY_COLORS[c][dark ? 'dark' : 'light']}"></i>${a.counts[c]}</span>`
				)
				.join('')}</span>`
		: '';
	return `<span class="call-marker-body" aria-hidden="true"><span class="call-marker-device"><span class="call-device call-device-${kind}">${pie}${keys}</span></span>${stats}</span>${decorated || kind === 'mobile' ? `<span class="call-marker-name" aria-hidden="true">${escapeHtml(name)}</span>` : ''}`;
}

/** Suppress persistent labels/counts when they would collide with another marker. */
export function decorationsFit(
	points: { x: number; y: number }[],
	i: number,
	width: number,
	compact: boolean
): boolean {
	const p = points[i];
	return (
		!compact &&
		width >= 640 &&
		p.x >= 90 &&
		p.x <= width - 90 &&
		points.every((q, j) => i === j || Math.abs(q.x - p.x) > 180 || Math.abs(q.y - p.y) > 115)
	);
}
