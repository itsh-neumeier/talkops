<script lang="ts">
	// Side panel of an extension (UniFi Talk style): overview, devices and
	// the manage actions. The extension page keeps the full editors.
	import {
		api,
		type Extension,
		type ExtensionWithDevices,
		type PhoneNumber,
		type User
	} from '#lib/api.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import Collapsible from './Collapsible.svelte';
	import ErrorBox from './ErrorBox.svelte';
	import InfoList from './InfoList.svelte';
	import PanelAction from './PanelAction.svelte';
	import SidePanel from './SidePanel.svelte';
	import { ICONS } from './panelIcons.ts';

	let {
		extension,
		users,
		numbers,
		onchanged,
		onclose
	}: {
		extension: Extension;
		users: User[];
		numbers: PhoneNumber[];
		onchanged: () => void;
		onclose: () => void;
	} = $props();

	let detail = $state<ExtensionWithDevices | null>(null);
	let error = $state('');
	let tab = $state('overview');
	const tabs = [
		{ id: 'overview', label: t('panel.overview'), icon: ICONS.overview },
		{ id: 'devices', label: t('panel.devices'), icon: ICONS.devices },
		{ id: 'settings', label: t('panel.settings'), icon: ICONS.settings }
	];

	$effect(() => {
		const id = extension.id;
		detail = null;
		error = '';
		api
			.get<ExtensionWithDevices>(`/extensions/${id}`)
			.then((value) => {
				if (id === extension.id) detail = value;
			})
			.catch((err) => (error = errorMessage(err)));
	});

	const user = $derived(users.find((u) => u.id === extension.user_id)?.display_name ?? '—');
	const number = $derived(
		numbers.find((n) => n.id === extension.outbound_number_id)?.e164 ?? t('ext.defaultNumber')
	);
	const status = $derived(
		[
			extension.enabled ? t('common.enabled') : t('common.disabled'),
			extension.dnd ? t('panel.dnd') : '',
			extension.forward_all ? t('panel.forward', { target: extension.forward_all }) : '',
			extension.hide_caller_id ? 'CLIR' : ''
		]
			.filter(Boolean)
			.join(' · ')
	);

	async function remove() {
		if (!confirm(t('common.confirmDelete', { name: extension.display_name }))) return;
		try {
			await api.del(`/extensions/${extension.id}`);
			onchanged();
			onclose();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<SidePanel
	title={`${extension.number} ${extension.display_name}`}
	subtitle={user === '—' ? '' : user}
	{tabs}
	bind:tab
	{onclose}
>
	<ErrorBox {error} />
	{#if tab === 'overview'}
		<InfoList
			rows={[
				[t('ext.number'), extension.number],
				[t('ext.displayName'), extension.display_name],
				[t('ext.user'), user],
				[t('ext.outboundNumber'), number],
				[t('panel.state'), status],
				[t('panel.devices'), detail ? String(detail.devices.length) : '…']
			]}
		/>
	{:else if tab === 'devices'}
		{#if detail && detail.devices.length === 0}
			<p class="text-sm text-slate-500 dark:text-slate-400">{t('panel.noDevices')}</p>
		{:else if detail}
			<ul class="divide-y divide-slate-100 text-sm dark:divide-slate-800">
				{#each detail.devices as d (d.id)}
					<li class="flex items-center justify-between gap-3 py-2">
						<span class="min-w-0">
							<span class="block truncate font-medium">{d.name}</span>
							<span class="block text-xs text-slate-500 dark:text-slate-400"
								>{t(`kinds.${d.kind}` as MessageKey)} · {d.sip_username}</span
							>
						</span>
						{#if d.phone_id && can('phones.view')}
							<a
								class="shrink-0 text-xs text-teal-700 hover:underline dark:text-teal-300"
								href="/phones?panel={d.phone_id}">{t('nav.phones')}</a
							>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	{:else}
		<Collapsible title={t('panel.general')}>
			<InfoList
				rows={[
					[t('ext.number'), extension.number],
					[t('ext.displayName'), extension.display_name]
				]}
			/>
			<PanelAction
				label={t('panel.openPage')}
				hint={t('panel.hint.edit')}
				href="/extensions/{extension.id}"
			/>
		</Collapsible>
		{#if can('extensions.manage')}
			<Collapsible title={t('panel.manage')}>
				<PanelAction
					label={t('common.edit')}
					hint={t('panel.hint.edit')}
					href="/extensions/{extension.id}"
				/>
				<PanelAction
					label={t('common.delete')}
					hint={t('panel.hint.removeExtension')}
					danger
					onclick={remove}
				/>
			</Collapsible>
		{/if}
	{/if}
</SidePanel>
