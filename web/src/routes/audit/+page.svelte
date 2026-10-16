<script lang="ts">
	import AuditExportSettings from '#lib/components/AuditExportSettings.svelte';
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import { onDestroy, onMount } from 'svelte';
	import { api, type AuditEntry } from '#lib/api.ts';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const PAGE = 50;
	type Sort = 'time' | 'user' | 'action' | 'entity';
	type Facets = {
		actions: string[];
		entity_types: string[];
		users: { id: string; username: string }[];
	};

	let tab = $state<'entries' | 'export'>('entries');
	let entries = $state<AuditEntry[] | null>(null);
	let total = $state(0);
	let offset = $state(0);
	let error = $state('');
	let facets = $state<Facets>({ actions: [], entity_types: [], users: [] });
	let filters = $state({
		from: '',
		to: '',
		user_id: '',
		action: '',
		entity_type: '',
		search: ''
	});
	let sort = $state<Sort>('time');
	let order = $state<'desc' | 'asc'>('desc');
	let generation = 0;
	let typing: ReturnType<typeof setTimeout> | undefined;

	/** The filter as query parameters, without paging. */
	function filterQuery(): URLSearchParams {
		const q = new URLSearchParams();
		// Dates are local days; `to` includes the whole day.
		if (filters.from) q.set('since', new Date(`${filters.from}T00:00:00`).toISOString());
		if (filters.to) {
			const end = new Date(`${filters.to}T00:00:00`);
			end.setDate(end.getDate() + 1);
			q.set('until', end.toISOString());
		}
		for (const key of ['user_id', 'action', 'entity_type', 'search'] as const) {
			const value = filters[key].trim();
			if (value) q.set(key, value);
		}
		q.set('sort', sort);
		q.set('order', order);
		return q;
	}

	async function load() {
		const run = ++generation;
		const q = filterQuery();
		q.set('limit', String(PAGE));
		q.set('offset', String(offset));
		try {
			const res = await api.getPage<AuditEntry>(`/audit?${q}`);
			if (run !== generation) return;
			entries = res.items;
			total = res.total;
			error = '';
		} catch (err) {
			if (run === generation) error = errorMessage(err);
		}
	}

	function refilter() {
		offset = 0;
		load();
	}

	function typed() {
		clearTimeout(typing);
		typing = setTimeout(refilter, 300);
	}

	function reset() {
		filters = { from: '', to: '', user_id: '', action: '', entity_type: '', search: '' };
		sort = 'time';
		order = 'desc';
		refilter();
	}

	function sortBy(column: Sort) {
		if (sort === column) order = order === 'desc' ? 'asc' : 'desc';
		else {
			sort = column;
			order = column === 'time' ? 'desc' : 'asc';
		}
		refilter();
	}

	function go(delta: number) {
		offset = Math.max(0, offset + delta * PAGE);
		load();
	}

	const pages = $derived(Math.max(1, Math.ceil(total / PAGE)));
	const pageNo = $derived(Math.floor(offset / PAGE) + 1);
	const downloadHref = (format: 'csv' | 'json') => {
		const q = filterQuery();
		q.set('format', format);
		return `/api/v1/audit/export?${q}`;
	};
	const ariaSort = (column: Sort) =>
		sort === column ? (order === 'asc' ? 'ascending' : 'descending') : 'none';
	const arrow = (column: Sort) => (sort === column ? (order === 'asc' ? ' ▲' : ' ▼') : '');

	onMount(() => {
		load();
		api
			.get<Facets>('/audit/facets')
			.then((f) => (facets = f))
			.catch(() => {});
	});
	onDestroy(() => clearTimeout(typing));
</script>

<div class="space-y-4">
	<PageHeader title={t('nav.audit')} />
	{#if can('audit.manage')}
		<Tabs
			tabs={[
				{ id: 'entries', label: t('audit.tabEntries') },
				{ id: 'export', label: t('audit.tabExport') }
			]}
			bind:value={tab}
		/>
	{/if}

	{#if tab === 'export' && can('audit.manage')}
		<AuditExportSettings />
	{:else}
		<ErrorBox {error} />
		<div class="card grid grid-cols-1 gap-3 p-4 sm:grid-cols-2 lg:grid-cols-3">
			<div>
				<label for="audit-from">{t('audit.from')}</label>
				<input
					id="audit-from"
					class="input"
					type="date"
					bind:value={filters.from}
					onchange={refilter}
				/>
			</div>
			<div>
				<label for="audit-to">{t('audit.to')}</label>
				<input
					id="audit-to"
					class="input"
					type="date"
					bind:value={filters.to}
					onchange={refilter}
				/>
			</div>
			<div>
				<label for="audit-user">{t('audit.user')}</label>
				<select id="audit-user" class="input" bind:value={filters.user_id} onchange={refilter}>
					<option value="">{t('audit.allUsers')}</option>
					{#each facets.users as u (u.id)}<option value={u.id}>{u.username}</option>{/each}
				</select>
			</div>
			<div>
				<label for="audit-action">{t('audit.action')}</label>
				<select id="audit-action" class="input" bind:value={filters.action} onchange={refilter}>
					<option value="">{t('audit.allActions')}</option>
					{#each facets.actions as a (a)}<option value={a}>{a}</option>{/each}
				</select>
			</div>
			<div>
				<label for="audit-entity">{t('audit.entity')}</label>
				<select
					id="audit-entity"
					class="input"
					bind:value={filters.entity_type}
					onchange={refilter}
				>
					<option value="">{t('audit.allEntities')}</option>
					{#each facets.entity_types as e (e)}<option value={e}>{e}</option>{/each}
				</select>
			</div>
			<div>
				<label for="audit-search">{t('common.search')}</label>
				<input
					id="audit-search"
					class="input"
					type="search"
					placeholder={t('audit.searchPlaceholder')}
					bind:value={filters.search}
					oninput={typed}
				/>
			</div>
			<div class="flex flex-wrap items-center gap-2 sm:col-span-2 lg:col-span-3">
				<span class="text-sm text-slate-500" role="status">{t('audit.count', { n: total })}</span>
				<button type="button" class="btn btn-sm" onclick={reset}>{t('audit.reset')}</button>
				<span class="grow"></span>
				<a class="btn btn-sm" href={downloadHref('csv')} download title={t('audit.downloadHint')}
					>⬇ CSV</a
				>
				<a class="btn btn-sm" href={downloadHref('json')} download title={t('audit.downloadHint')}
					>⬇ JSON</a
				>
			</div>
		</div>

		<div class="card scroll-x">
			<table class="table">
				<thead>
					<tr>
						<th aria-sort={ariaSort('time')}
							><button type="button" onclick={() => sortBy('time')}
								>{t('audit.time')}{arrow('time')}</button
							></th
						>
						<th aria-sort={ariaSort('user')}
							><button type="button" onclick={() => sortBy('user')}
								>{t('audit.user')}{arrow('user')}</button
							></th
						>
						<th aria-sort={ariaSort('action')}
							><button type="button" onclick={() => sortBy('action')}
								>{t('audit.action')}{arrow('action')}</button
							></th
						>
						<th class="col-opt" aria-sort={ariaSort('entity')}
							><button type="button" onclick={() => sortBy('entity')}
								>{t('audit.entity')}{arrow('entity')}</button
							></th
						>
						<th class="col-opt-lg">{t('audit.details')}</th>
					</tr>
				</thead>
				<tbody>
					{#each entries ?? [] as e (e.id)}
						<tr>
							<td class="whitespace-nowrap">{formatDateTime(e.created_at)}</td>
							<td
								>{e.username ?? '—'}
								<div class="text-xs text-slate-500">{e.ip ?? ''}</div></td
							>
							<td>{e.action}</td>
							<td class="col-opt"
								>{e.entity_type}
								{#if e.entity_id}<div class="text-xs break-all text-slate-500">
										{e.entity_id}
									</div>{/if}</td
							>
							<td
								class="col-opt-lg max-w-md truncate font-mono text-xs"
								title={JSON.stringify(e.details)}>{JSON.stringify(e.details)}</td
							>
						</tr>
					{:else}
						{#if entries === null}<SkeletonRows cols={5} />{:else}<EmptyRow cols={5} />{/if}
					{/each}
				</tbody>
			</table>
		</div>
		{#if pages > 1}
			<nav
				class="flex items-center justify-end gap-2"
				aria-label={t('audit.page', { page: pageNo, pages })}
			>
				<button type="button" class="btn btn-sm" disabled={offset === 0} onclick={() => go(-1)}
					>{t('audit.prev')}</button
				>
				<span class="text-sm">{t('audit.page', { page: pageNo, pages })}</span>
				<button type="button" class="btn btn-sm" disabled={pageNo >= pages} onclick={() => go(1)}
					>{t('audit.next')}</button
				>
			</nav>
		{/if}
	{/if}
</div>
