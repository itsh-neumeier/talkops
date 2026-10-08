<script lang="ts">
	// Smart Attendants: list with a short outline of each flow; editing
	// happens in the flow editor.
	import { api, type Attendant, type FlowNode } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { loadTargets, targets } from '#lib/destinations.svelte.ts';
	import { ICONS, typeKey, walk } from '#lib/flow.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let error = $state('');

	/** Step types used, in order of first appearance. */
	function outline(flow: FlowNode) {
		const seen = new Set<FlowNode['type']>();
		for (const n of walk(flow)) seen.add(n.type);
		return [...seen];
	}

	async function remove(a: Attendant) {
		if (!confirm(t('common.confirmDelete', { name: a.name }))) return;
		try {
			await api.del(`/attendants/${a.id}`);
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-3">
	<div class="flex items-center justify-between gap-2">
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('flow.listHint')}</p>
		{#if hasRole('admin')}<a class="btn btn-primary" href="/routing/attendants/new"
				>{t('flow.new')}</a
			>{/if}
	</div>
	<ErrorBox {error} />
	{#if targets.menus.length === 0}
		<div class="card"><p class="text-sm text-slate-500">{t('routing.none')}</p></div>
	{/if}
	{#each targets.menus as a (a.id)}
		<section class="card flex flex-wrap items-center justify-between gap-3">
			<div class="min-w-0">
				<h3 class="font-medium"><span class="font-mono">{a.number ?? ''}</span> {a.name}</h3>
				<p class="mt-1 flex flex-wrap items-center gap-1.5 text-xs text-slate-500">
					{t('flow.steps', { n: walk(a.flow).length })}
					{#each outline(a.flow) as type (type)}
						<span
							class="inline-flex items-center gap-1 rounded-full bg-slate-100 px-2 py-0.5 dark:bg-slate-800"
						>
							<svg
								viewBox="0 0 24 24"
								class="h-3.5 w-3.5"
								fill="none"
								stroke="currentColor"
								stroke-width="1.8"
								stroke-linecap="round"
								stroke-linejoin="round"><path d={ICONS[type]} /></svg
							>
							{t(typeKey(type))}
						</span>
					{/each}
				</p>
			</div>
			<span class="space-x-1">
				<a class="btn btn-sm" href="/routing/attendants/{a.id}"
					>{hasRole('admin') ? t('common.edit') : t('flow.view')}</a
				>
				{#if hasRole('admin')}
					<button class="btn btn-sm btn-danger" onclick={() => remove(a)}
						>{t('common.delete')}</button
					>
				{/if}
			</span>
		</section>
	{/each}
</div>
