import type { MapArea } from '../api.ts';
import { CATEGORIES, type CategoryCounts } from './map.ts';

export interface MarkerBox {
	id: string;
	x: number;
	y: number;
	width: number;
	height: number;
	/** Persistent label below the centered icon. */
	labelHeight: number;
}

export interface MarkerCluster extends Omit<MarkerBox, 'id'> {
	members: MarkerBox[];
}

export const CLUSTER_SIZE = 64;

/** Include icon shadows, handset, base, labels and focus outlines in collision checks. */
export function boxesOverlap(a: Omit<MarkerBox, 'id'>, b: Omit<MarkerBox, 'id'>): boolean {
	const gap = 10;
	return (
		Math.abs(a.x - b.x) < (a.width + b.width) / 2 + gap &&
		a.y - a.height / 2 < b.y + b.height / 2 + b.labelHeight + gap &&
		b.y - b.height / 2 < a.y + a.height / 2 + a.labelHeight + gap
	);
}

/**
 * Merge intersecting screen footprints until the rendered clusters also fit.
 * Recompute from the original points on every zoom; do not move API coordinates.
 * Stable ID order makes the result independent of API count order.
 */
export function clusterMarkers(points: MarkerBox[]): MarkerCluster[] {
	const groups = new Set<MarkerCluster>();
	// A small screen grid avoids rescanning every distant marker after each merge.
	const grid = new Map<string, Set<MarkerCluster>>();
	function cells(p: MarkerCluster, margin = 0): string[] {
		const keys: string[] = [];
		for (
			let x = Math.floor((p.x - p.width / 2 - margin) / 128);
			x <= Math.floor((p.x + p.width / 2 + margin) / 128);
			x++
		) {
			for (
				let y = Math.floor((p.y - p.height / 2 - margin) / 128);
				y <= Math.floor((p.y + p.height / 2 + p.labelHeight + margin) / 128);
				y++
			) {
				keys.push(`${x}:${y}`);
			}
		}
		return keys;
	}
	function remove(p: MarkerCluster) {
		groups.delete(p);
		for (const key of cells(p)) {
			const bucket = grid.get(key)!;
			bucket.delete(p);
			if (!bucket.size) grid.delete(key);
		}
	}
	for (const point of [...points].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))) {
		let group: MarkerCluster = { ...point, members: [point] };
		while (true) {
			const candidates = new Set(cells(group, 10).flatMap((key) => [...(grid.get(key) ?? [])]));
			const hit = [...candidates].find((p) => boxesOverlap(group, p));
			if (!hit) break;
			remove(hit);
			const members = [...group.members, ...hit.members];
			group = {
				members,
				x: members.reduce((sum, p) => sum + p.x, 0) / members.length,
				y: members.reduce((sum, p) => sum + p.y, 0) / members.length,
				width: CLUSTER_SIZE,
				height: CLUSTER_SIZE,
				labelHeight: 0
			};
		}
		groups.add(group);
		for (const key of cells(group)) {
			if (!grid.has(key)) grid.set(key, new Set());
			grid.get(key)!.add(group);
		}
	}
	return [...groups];
}

/** No call samples are combined or copied into a cluster preview. */
export function clusterCounts(areas: MapArea[]): CategoryCounts {
	return Object.fromEntries(
		CATEGORIES.map((c) => [c, areas.reduce((sum, a) => sum + a.counts[c], 0)])
	) as CategoryCounts;
}
