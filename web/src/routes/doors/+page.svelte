<script lang="ts">
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onDestroy, onMount } from 'svelte';
	import { api, type DoorEvent, type DoorStation } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let stations = $state<DoorStation[] | null>(null);
	let events = $state<DoorEvent[]>([]);
	let error = $state('');
	let info = $state('');
	let live = $state<Record<string, number>>({});
	let timer: ReturnType<typeof setInterval> | undefined;
	let tick = 0;

	async function load() {
		try {
			stations = await api.get<DoorStation[]>('/door-stations');
			events = await api.get<DoorEvent[]>('/door-events?limit=30');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	onMount(() => {
		load();
		// Live pictures every 2 s, the event log every 10 s.
		timer = setInterval(() => {
			tick++;
			for (const id of Object.keys(live)) live[id] = Date.now();
			if (tick % 5 === 0) load();
		}, 2000);
	});
	onDestroy(() => clearInterval(timer));

	const stationName = (id: string) => stations?.find((s) => s.id === id)?.name ?? '—';

	function toggleLive(s: DoorStation) {
		if (live[s.id]) delete live[s.id];
		else live[s.id] = Date.now();
	}

	async function open(s: DoorStation, door: number) {
		error = '';
		info = '';
		if (!confirm(t('door.openConfirm', { name: s.name }))) return;
		try {
			await api.post(`/door-stations/${s.id}/open`, { door });
			info = t('door.opened', { name: s.name });
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function detail(e: DoorEvent): string {
		const d = e.detail as Record<string, unknown>;
		const by = d.by as Record<string, unknown> | undefined;
		const parts: string[] = [];
		if (typeof d.dialed === 'string' && d.dialed) parts.push(d.dialed);
		if (typeof d.door === 'number' && d.door > 1) parts.push(t('door.doorN', { n: d.door }));
		if (by?.user) parts.push(String(by.user));
		else if (by?.token) parts.push(t('door.byToken'));
		else if (by?.extension_id) parts.push(t('door.byPhone'));
		if (d.ok === false) parts.push(t('door.failed'));
		if (typeof d.model === 'string' && d.model) parts.push(d.model);
		return parts.join(' · ');
	}
</script>

<div class="space-y-4">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h1>{t('nav.doors')}</h1>
		{#if hasRole('admin')}
			<a class="btn" href="/door-stations">{t('door.setup')}</a>
		{/if}
	</div>
	<ErrorBox {error} />
	{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400" role="status">{info}</p>{/if}

	{#if stations && stations.length === 0}
		<div class="card text-sm text-slate-600 dark:text-slate-300">{t('door.none')}</div>
	{/if}

	<div class="grid gap-4 md:grid-cols-2">
		{#each stations ?? [] as s (s.id)}
			<div class="card space-y-3">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<div>
						<h2>{s.name}</h2>
						{#if s.model}<p class="text-xs text-slate-500">{s.model}</p>{/if}
					</div>
					{#if s.events_enabled && hasRole('admin')}
						<span class="badge {s.online ? 'badge-ok' : 'badge-bad'}"
							>{s.online ? t('door.online') : t('door.offline')}</span
						>
					{/if}
					{#if !s.enabled}<span class="badge badge-muted">{t('common.disabled')}</span>{/if}
				</div>
				{#if live[s.id]}
					<img
						class="aspect-video w-full rounded-md bg-slate-200 object-cover dark:bg-slate-800"
						src="/api/v1/door-stations/{s.id}/snapshot?t={live[s.id]}"
						alt={t('door.liveAlt', { name: s.name })}
					/>
				{/if}
				<div class="flex flex-wrap gap-2">
					<button class="btn btn-primary" onclick={() => open(s, 1)}
						>🔓 {s.doors > 1 ? t('door.openN', { n: 1 }) : t('door.open')}</button
					>
					{#if s.doors > 1}
						<button class="btn btn-primary" onclick={() => open(s, 2)}
							>🔓 {t('door.openN', { n: 2 })}</button
						>
					{/if}
					<button class="btn" aria-pressed={!!live[s.id]} onclick={() => toggleLive(s)}
						>{live[s.id] ? t('door.liveStop') : t('door.live')}</button
					>
				</div>
			</div>
		{:else}
			{#if !net.settled}<section class="card"><Skeleton lines={4} /></section>{/if}
		{/each}
	</div>

	<div class="card overflow-x-auto">
		<h2 class="mb-2">{t('door.events')}</h2>
		{#if events.length === 0}
			<p class="text-sm text-slate-500">{t('door.noEvents')}</p>
		{:else}
			<table class="table">
				<thead>
					<tr
						><th>{t('calls.time')}</th><th>{t('nav.doors')}</th><th>{t('door.event')}</th><th
						></th><th><span class="sr-only">{t('door.snapshot')}</span></th></tr
					>
				</thead>
				<tbody>
					{#each events as e (e.id)}
						<tr>
							<td class="whitespace-nowrap">{formatDateTime(e.created_at)}</td>
							<td>{stationName(e.door_station_id)}</td>
							<td>{t(`door.kind.${e.kind}` as MessageKey)}</td>
							<td class="text-sm text-slate-500">{detail(e)}</td>
							<td>
								{#if e.has_snapshot}
									<a href="/api/v1/door-events/{e.id}/snapshot" target="_blank" rel="noopener"
										><img
											class="h-12 w-20 rounded object-cover"
											src="/api/v1/door-events/{e.id}/snapshot"
											alt={t('door.snapshot')}
											loading="lazy"
										/></a
									>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>
