<script lang="ts">
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { api, type Extension, type PhoneNumber, type User } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import ExtensionForm from '#lib/components/ExtensionForm.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { loadTargets } from '#lib/destinations.svelte.ts';
	import ExtensionInspector from '#lib/components/ExtensionInspector.svelte';
	import { panelParam, rowClick, setPanelParam } from '#lib/panel.ts';

	let extensions = $state<Extension[]>([]);
	let users = $state<User[]>([]);
	let numbers = $state<PhoneNumber[]>([]);
	let error = $state('');
	let open = $state(false);

	async function load() {
		// Names of number destinations ("now: group …") in the edit dialog.
		if (can('extensions.manage')) loadTargets().catch(() => {});
		try {
			[extensions, users, numbers] = await Promise.all([
				api.get<Extension[]>('/extensions'),
				api.get<User[]>('/users'),
				api.get<PhoneNumber[]>('/numbers')
			]);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	// Side panel of one extension (`?panel=<id>`).
	let panel = $state<string | null>(panelParam());
	const panelExtension = $derived(extensions.find((e) => e.id === panel));
	function openPanel(id: string | null) {
		panel = id;
		setPanelParam(id);
	}

	const userName = (id: string | null) => users.find((u) => u.id === id)?.display_name ?? '—';
	const numberOf = (id: string | null) => numbers.find((n) => n.id === id)?.e164;
</script>

<div class="space-y-4 {panel ? 'lg:pr-[26rem]' : ''}">
	<PageHeader title={t('nav.extensions')}>
		{#snippet actions()}
			{#if can('extensions.manage')}<button class="btn btn-primary" onclick={() => (open = true)}
					>{t('ext.new')}</button
				>{/if}
		{/snippet}
	</PageHeader>
	<ErrorBox {error} />
	<div class="card scroll-x">
		<table class="table">
			<thead>
				<tr>
					<th>{t('ext.number')}</th>
					<th>{t('ext.displayName')}</th>
					<th class="col-opt">{t('ext.user')}</th>
					<th class="col-opt-lg">{t('ext.outboundNumber')}</th>
				</tr>
			</thead>
			<tbody>
				{#each extensions as ext (ext.id)}
					<tr
						class="cursor-pointer {panel === ext.id ? 'bg-teal-50 dark:bg-teal-950/40' : ''}"
						onclick={(e) => rowClick(e, () => openPanel(ext.id))}
					>
						<td class="font-mono"
							><a
								class="hover:underline"
								href="/extensions/{ext.id}"
								data-panel-link
								aria-current={panel === ext.id ? 'true' : undefined}>{ext.number}</a
							></td
						>
						<td>
							<a class="hover:underline" href="/extensions/{ext.id}" data-panel-link
								>{ext.display_name}</a
							>
							{#if !ext.enabled}<span class="badge badge-muted">{t('common.disabled')}</span>{/if}
							{#if ext.hide_caller_id}<span class="badge badge-warn">CLIR</span>{/if}
						</td>
						<td class="col-opt">{userName(ext.user_id)}</td>
						<td class="col-opt-lg font-mono"
							>{numberOf(ext.outbound_number_id) ?? t('ext.defaultNumber')}</td
						>
					</tr>
				{:else}
					{#if !net.settled}<SkeletonRows cols={4} />{:else}<EmptyRow cols={4} />{/if}
				{/each}
			</tbody>
		</table>
	</div>
</div>

<Modal title={t('ext.new')} bind:open>
	<ExtensionForm
		{users}
		{numbers}
		onsaved={async () => {
			open = false;
			await load();
		}}
		oncancel={() => (open = false)}
	/>
</Modal>

{#if panelExtension}
	<ExtensionInspector
		extension={panelExtension}
		{users}
		{numbers}
		onchanged={load}
		onclose={() => openPanel(null)}
	/>
{/if}
