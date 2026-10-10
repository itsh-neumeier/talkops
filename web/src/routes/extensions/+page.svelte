<script lang="ts">
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { api, type Extension, type PhoneNumber, type User } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import ExtensionForm from '#lib/components/ExtensionForm.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { loadTargets } from '#lib/destinations.svelte.ts';

	let extensions = $state<Extension[]>([]);
	let users = $state<User[]>([]);
	let numbers = $state<PhoneNumber[]>([]);
	let error = $state('');
	let open = $state(false);

	async function load() {
		// Names of number destinations ("now: group …") in the edit dialog.
		if (hasRole('admin')) loadTargets().catch(() => {});
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

	const userName = (id: string | null) => users.find((u) => u.id === id)?.display_name ?? '—';
	const numberOf = (id: string | null) => numbers.find((n) => n.id === id)?.e164;
</script>

<div class="space-y-4">
	<div class="flex items-center justify-between">
		<h1>{t('nav.extensions')}</h1>
		{#if hasRole('admin')}<button class="btn btn-primary" onclick={() => (open = true)}
				>{t('ext.new')}</button
			>{/if}
	</div>
	<ErrorBox {error} />
	<div class="card overflow-x-auto">
		<table class="table">
			<thead>
				<tr>
					<th>{t('ext.number')}</th>
					<th>{t('ext.displayName')}</th>
					<th>{t('ext.user')}</th>
					<th>{t('ext.outboundNumber')}</th>
				</tr>
			</thead>
			<tbody>
				{#each extensions as ext (ext.id)}
					<tr>
						<td class="font-mono"
							><a class="hover:underline" href="/extensions/{ext.id}">{ext.number}</a></td
						>
						<td>
							<a class="hover:underline" href="/extensions/{ext.id}">{ext.display_name}</a>
							{#if !ext.enabled}<span class="badge badge-muted">{t('common.disabled')}</span>{/if}
							{#if ext.hide_caller_id}<span class="badge badge-warn">CLIR</span>{/if}
						</td>
						<td>{userName(ext.user_id)}</td>
						<td class="font-mono">{numberOf(ext.outbound_number_id) ?? t('ext.defaultNumber')}</td>
					</tr>
				{:else}
					{#if !net.settled}<SkeletonRows cols={4} />{/if}
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
