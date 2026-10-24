<script lang="ts">
	// Side panel of a user (UniFi Talk style): overview, extensions of the
	// user and the manage actions. The actions use the dialogs of the page.
	import { api, type Extension, type User } from '#lib/api.ts';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import Collapsible from './Collapsible.svelte';
	import InfoList from './InfoList.svelte';
	import PanelAction from './PanelAction.svelte';
	import SidePanel from './SidePanel.svelte';
	import { ICONS } from './panelIcons.ts';

	let {
		user,
		role,
		onedit,
		onpassword,
		onresettotp,
		onremove,
		onclose
	}: {
		user: User;
		/** Display name of the role. */
		role: string;
		onedit: () => void;
		onpassword: () => void;
		onresettotp: () => void;
		onremove: () => void;
		onclose: () => void;
	} = $props();

	let tab = $state('overview');
	const tabs = [
		{ id: 'overview', label: t('panel.overview'), icon: ICONS.overview },
		{ id: 'extensions', label: t('panel.extensions'), icon: ICONS.extensions },
		{ id: 'settings', label: t('panel.settings'), icon: ICONS.settings }
	];
	let extensions = $state<Extension[] | null>(null);
	$effect(() => {
		if (!can('extensions.view')) return;
		api
			.get<Extension[]>('/extensions')
			.then((all) => (extensions = all))
			.catch(() => (extensions = []));
	});
	const own = $derived((extensions ?? []).filter((e) => e.user_id === user.id));
</script>

<SidePanel title={user.display_name} subtitle={user.username} {tabs} bind:tab {onclose}>
	{#if tab === 'overview'}
		<InfoList
			rows={[
				[t('users.username'), user.username],
				[t('users.displayName'), user.display_name],
				[t('users.email'), user.email ?? '—'],
				[t('users.role'), role],
				[t('panel.authSource'), user.auth_source],
				[t('users.lastLogin'), formatDateTime(user.last_login_at)],
				['2FA', user.totp_enabled ? t('common.enabled') : t('common.disabled')],
				[t('panel.state'), user.enabled ? t('common.enabled') : t('common.disabled')]
			]}
		/>
	{:else if tab === 'extensions'}
		{#if !can('extensions.view') || (extensions && own.length === 0)}
			<p class="text-sm text-slate-500 dark:text-slate-400">{t('panel.noExtensions')}</p>
		{:else}
			<ul class="divide-y divide-slate-100 text-sm dark:divide-slate-800">
				{#each own as e (e.id)}
					<li class="py-2">
						<a class="hover:underline" href="/extensions?panel={e.id}"
							><span class="font-mono">{e.number}</span> {e.display_name}</a
						>
					</li>
				{/each}
			</ul>
		{/if}
	{:else}
		<Collapsible title={t('panel.general')}>
			<InfoList
				rows={[
					[t('users.username'), user.username],
					[t('users.role'), role]
				]}
			/>
		</Collapsible>
		{#if can('users.manage')}
			<Collapsible title={t('panel.manage')}>
				<PanelAction label={t('common.edit')} hint={t('panel.hint.edit')} onclick={onedit} />
				<PanelAction
					label={t('users.setPassword')}
					hint={t('panel.hint.password')}
					onclick={onpassword}
				/>
				{#if user.totp_enabled}
					<PanelAction label={t('mfa.reset')} hint={t('panel.hint.mfa')} onclick={onresettotp} />
				{/if}
				<PanelAction
					label={t('common.delete')}
					hint={t('panel.hint.removeUser')}
					danger
					onclick={onremove}
				/>
			</Collapsible>
		{/if}
	{/if}
</SidePanel>
