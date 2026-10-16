<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		api,
		type Permission,
		type PermissionInfo,
		type RoleRecord,
		type RolesOverview
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { isAdmin } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { roleName } from '#lib/roles.ts';

	/** Called after roles were saved, created or deleted. */
	let { onchange }: { onchange?: () => void } = $props();

	let catalog = $state<PermissionInfo[]>([]);
	let roles = $state<RoleRecord[] | null>(null);
	/** Edited permission sets by role id (only roles with changes). */
	const drafts = new SvelteMap<string, Set<Permission>>();
	let error = $state('');
	let info = $state('');
	let saving = $state(false);
	let dialog = $state(false);
	let form = $state({
		id: '',
		name: '',
		description: '',
		directory_group: '',
		copy: ''
	});

	const editable = $derived(isAdmin());
	const areas = $derived(
		catalog.reduce<{ area: string; items: PermissionInfo[] }[]>((list, p) => {
			const last = list.at(-1);
			if (last && last.area === p.area) last.items.push(p);
			else list.push({ area: p.area, items: [p] });
			return list;
		}, [])
	);

	async function load() {
		try {
			const overview = await api.get<RolesOverview>('/roles');
			catalog = overview.permissions ?? [];
			roles = overview.roles ?? [];
			drafts.clear();
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	const isAdminRole = (r: RoleRecord) => r.builtin === 'admin';

	function current(r: RoleRecord): Set<Permission> {
		return drafts.get(r.id) ?? new Set(r.permissions);
	}

	function has(r: RoleRecord, p: Permission): boolean {
		return isAdminRole(r) || current(r).has(p);
	}

	function same(a: Set<Permission>, b: Permission[]): boolean {
		return a.size === b.length && b.every((p) => a.has(p));
	}

	function setDraft(r: RoleRecord, next: Set<Permission>) {
		if (same(next, r.permissions)) drafts.delete(r.id);
		else drafts.set(r.id, next);
		info = '';
	}

	function toggle(r: RoleRecord, p: Permission) {
		const next = new Set(current(r));
		if (next.has(p)) next.delete(p);
		else next.add(p);
		setDraft(r, next);
	}

	function toggleArea(r: RoleRecord, items: PermissionInfo[]) {
		const next = new Set(current(r));
		const all = items.every((i) => next.has(i.key));
		for (const i of items) {
			if (all) next.delete(i.key);
			else next.add(i.key);
		}
		setDraft(r, next);
	}

	function areaState(r: RoleRecord, items: PermissionInfo[]): 'all' | 'some' | 'none' {
		const n = items.filter((i) => has(r, i.key)).length;
		return n === items.length ? 'all' : n ? 'some' : 'none';
	}

	async function saveAll() {
		if (!roles) return;
		saving = true;
		error = '';
		try {
			for (const r of roles) {
				const draft = drafts.get(r.id);
				if (!draft) continue;
				await api.put(`/roles/${r.id}`, {
					name: r.name,
					description: r.description,
					directory_group: r.directory_group,
					permissions: catalog.map((c) => c.key).filter((k) => draft.has(k))
				});
			}
			await load();
			info = t('roles.saved');
			onchange?.();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			saving = false;
		}
	}

	function openNew() {
		form = { id: '', name: '', description: '', directory_group: '', copy: '' };
		dialog = true;
	}

	function openEdit(r: RoleRecord) {
		form = {
			id: r.id,
			name: r.name,
			description: r.description,
			directory_group: r.directory_group,
			copy: ''
		};
		dialog = true;
	}

	async function saveRole(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			if (form.id) {
				const r = roles!.find((x) => x.id === form.id)!;
				await api.put(`/roles/${r.id}`, {
					name: form.name,
					description: form.description,
					directory_group: form.directory_group,
					permissions: [...current(r)]
				});
			} else {
				const source = roles!.find((x) => x.id === form.copy);
				const permissions = source
					? isAdminRole(source)
						? catalog.map((c) => c.key)
						: [...current(source)]
					: [];
				await api.post('/roles', {
					name: form.name,
					description: form.description,
					directory_group: form.directory_group,
					permissions
				});
			}
			dialog = false;
			// Keep other unsaved changes: reload only the role list fields.
			const kept = new Map(drafts);
			await load();
			for (const [id, set] of kept) if (id !== form.id) drafts.set(id, set);
			onchange?.();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(r: RoleRecord) {
		if (!confirm(t('roles.deleteConfirm', { name: roleName(r) }))) return;
		try {
			await api.del(`/roles/${r.id}`);
			await load();
			onchange?.();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-4">
	<div class="card space-y-2">
		<div class="flex flex-wrap items-start justify-between gap-3">
			<div class="min-w-0 flex-1">
				<h2>{t('roles.title')}</h2>
				<p class="mt-1 text-sm text-slate-600 dark:text-slate-300">{t('roles.intro')}</p>
				{#if !editable}<p class="hint">{t('roles.readOnly')}</p>{/if}
			</div>
			{#if editable}
				<button class="btn btn-primary" onclick={openNew}>{t('roles.new')}</button>
			{/if}
		</div>
		<ErrorBox {error} />
		{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400" role="status">
				{info}
			</p>{/if}
	</div>

	{#if !roles}
		<div class="card"><Skeleton lines={8} /></div>
	{:else}
		<div class="card scroll-x max-h-[calc(100dvh-var(--header-h)-7rem)] overflow-y-auto p-0">
			<table class="role-matrix w-full border-separate border-spacing-0 text-left text-sm">
				<thead>
					<tr>
						<th
							class="sticky top-0 left-0 z-30 min-w-[14rem] border-b border-slate-200 bg-white p-3 align-bottom font-medium text-slate-500 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-400"
						>
							{t('roles.permission')}
						</th>
						{#each roles as r (r.id)}
							<th
								class="sticky top-0 z-20 min-w-[8.5rem] border-b border-slate-200 bg-white p-3 text-center align-bottom dark:border-slate-800 dark:bg-slate-900"
							>
								<div class="font-semibold text-slate-900 dark:text-slate-100">{roleName(r)}</div>
								<div class="text-xs font-normal text-slate-500 dark:text-slate-400">
									{t('roles.users', { count: r.users })}
									{#if r.builtin}· {t('roles.builtin')}{/if}
								</div>
								{#if r.directory_group}
									<div
										class="mt-0.5 truncate font-mono text-[11px] font-normal text-slate-400"
										title={t('roles.directoryGroup')}
									>
										{r.directory_group}
									</div>
								{/if}
								{#if editable && !isAdminRole(r)}
									<div class="mt-1.5 flex justify-center gap-1">
										<button
											class="btn btn-sm"
											onclick={() => openEdit(r)}
											aria-label="{t('roles.edit')}: {roleName(r)}">{t('common.edit')}</button
										>
										{#if !r.builtin}
											<button
												class="btn btn-sm btn-danger"
												disabled={r.users > 0}
												title={r.users > 0 ? t('roles.inUse') : undefined}
												onclick={() => remove(r)}
												aria-label="{t('common.delete')}: {roleName(r)}">✕</button
											>
										{/if}
									</div>
								{/if}
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each areas as group (group.area)}
						<tr>
							<th
								scope="rowgroup"
								class="sticky left-0 z-10 border-b border-slate-200 bg-slate-50 px-3 pt-4 pb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase dark:border-slate-800 dark:bg-slate-950 dark:text-slate-400"
							>
								{t(`permArea.${group.area}` as MessageKey)}
							</th>
							{#each roles as r (r.id)}
								{@const state = areaState(r, group.items)}
								<td
									class="border-b border-slate-200 bg-slate-50 px-3 pt-4 pb-2 text-center dark:border-slate-800 dark:bg-slate-950"
								>
									{#if editable && !isAdminRole(r)}
										<input
											type="checkbox"
											class="h-4 w-4 accent-teal-600"
											checked={state === 'all'}
											indeterminate={state === 'some'}
											aria-label={t('roles.toggleArea', {
												area: t(`permArea.${group.area}` as MessageKey),
												role: roleName(r)
											})}
											onchange={() => toggleArea(r, group.items)}
										/>
									{/if}
								</td>
							{/each}
						</tr>
						{#each group.items as p (p.key)}
							<tr class="group/row">
								<th
									scope="row"
									class="sticky left-0 z-10 border-b border-slate-100 bg-white p-3 align-top font-normal transition-colors group-hover/row:bg-slate-50 dark:border-slate-800/60 dark:bg-slate-900 dark:group-hover/row:bg-slate-800/60"
								>
									<div
										class="flex items-center gap-1.5 font-medium text-slate-800 dark:text-slate-100"
									>
										{t(`perm.${p.key}` as MessageKey)}
										{#if p.sensitive}
											<span class="badge badge-warn" title={t('roles.sensitiveHint')}
												>{t('roles.sensitive')}</span
											>
										{/if}
									</div>
									<div class="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
										{t(`perm.${p.key}.hint` as MessageKey)}
									</div>
								</th>
								{#each roles as r (r.id)}
									{@const changed =
										drafts.has(r.id) && current(r).has(p.key) !== r.permissions.includes(p.key)}
									<td
										class="border-b border-slate-100 p-3 text-center align-middle transition-colors group-hover/row:bg-slate-50 dark:border-slate-800/60 dark:group-hover/row:bg-slate-800/60 {changed
											? 'bg-amber-50 dark:bg-amber-950/30'
											: ''}"
									>
										<input
											type="checkbox"
											class="h-4 w-4 accent-teal-600 disabled:opacity-60"
											checked={has(r, p.key)}
											disabled={!editable || isAdminRole(r)}
											title={isAdminRole(r) ? t('roles.adminAll') : undefined}
											aria-label="{t(`perm.${p.key}` as MessageKey)}: {roleName(r)}"
											onchange={() => toggle(r, p.key)}
										/>
									</td>
								{/each}
							</tr>
						{/each}
					{/each}
				</tbody>
			</table>
		</div>
	{/if}

	{#if drafts.size > 0}
		<div
			class="save-bar sticky bottom-3 z-20 flex flex-wrap items-center justify-between gap-2 rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 shadow-lg dark:border-amber-800 dark:bg-amber-950"
			role="status"
		>
			<span class="text-sm text-amber-900 dark:text-amber-200"
				>{t('roles.unsaved', { count: drafts.size })}</span
			>
			<div class="flex gap-2">
				<button class="btn btn-sm" disabled={saving} onclick={() => drafts.clear()}
					>{t('roles.discard')}</button
				>
				<button class="btn btn-sm btn-primary" disabled={saving} onclick={saveAll}
					>{t('common.save')}</button
				>
			</div>
		</div>
	{/if}
</div>

<Modal title={form.id ? t('roles.edit') : t('roles.new')} bind:open={dialog}>
	<form class="space-y-3" onsubmit={saveRole}>
		<ErrorBox {error} />
		<div>
			<label for="role-name">{t('roles.name')}</label>
			<input id="role-name" class="input" maxlength="64" bind:value={form.name} required />
		</div>
		<div>
			<label for="role-desc">{t('roles.description')} ({t('common.optional')})</label>
			<input id="role-desc" class="input" bind:value={form.description} />
		</div>
		<div>
			<label for="role-group">{t('roles.directoryGroup')} ({t('common.optional')})</label>
			<input id="role-group" class="input font-mono" bind:value={form.directory_group} />
			<p class="hint">{t('roles.directoryGroupHint')}</p>
		</div>
		{#if !form.id}
			<div>
				<label for="role-copy">{t('roles.copyFrom')}</label>
				<select id="role-copy" class="input" bind:value={form.copy}>
					<option value="">{t('roles.copyNone')}</option>
					{#each roles ?? [] as r (r.id)}<option value={r.id}>{roleName(r)}</option>{/each}
				</select>
			</div>
		{/if}
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (dialog = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
</Modal>
