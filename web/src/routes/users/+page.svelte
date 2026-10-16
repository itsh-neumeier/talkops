<script lang="ts">
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { api, type RoleRecord, type RolesOverview, type User } from '#lib/api.ts';
	import RoleEditor from '#lib/components/RoleEditor.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import { roleDescription, roleName, roleOf } from '#lib/roles.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { can, isAdmin } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let users = $state<User[]>([]);
	let roles = $state<RoleRecord[]>([]);
	let tab = $state<'users' | 'roles'>(
		typeof location !== 'undefined' && location.hash === '#roles' ? 'roles' : 'users'
	);
	let error = $state('');
	let editing = $state<Partial<User> & { password?: string; role_choice?: string }>({});
	let open = $state(false);
	let pwOpen = $state(false);
	let pwUser = $state<User | null>(null);
	let newPassword = $state('');
	/** Roles this user may give: admins all, others no administrator role. */
	const choices = $derived(roles.filter((r) => isAdmin() || r.builtin !== 'admin'));

	async function load() {
		try {
			[users, roles] = await Promise.all([
				api.get<User[]>('/users'),
				api
					.get<RolesOverview>('/roles')
					.then((o) => o.roles ?? [])
					.catch(() => [])
			]);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	function create() {
		editing = {
			username: '',
			display_name: '',
			email: '',
			role: 'user',
			role_choice: roles.find((r) => r.builtin === 'user')?.id ?? '',
			enabled: true,
			password: ''
		};
		open = true;
	}

	function edit(user: User) {
		editing = { ...user, role_choice: roleOf(user, roles)?.id ?? '' };
		open = true;
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			if (editing.id) {
				await api.put(`/users/${editing.id}`, {
					display_name: editing.display_name,
					email: editing.email,
					role: editing.role,
					role_id: editing.role_choice || null,
					enabled: editing.enabled
				});
			} else {
				await api.post('/users', {
					username: editing.username,
					display_name: editing.display_name,
					email: editing.email,
					role: editing.role,
					role_id: editing.role_choice || null,
					password: editing.password || null
				});
			}
			open = false;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(user: User) {
		if (!confirm(t('common.confirmDelete', { name: user.username }))) return;
		try {
			await api.del(`/users/${user.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function resetTotp(user: User) {
		if (!confirm(t('mfa.resetConfirm', { name: user.username }))) return;
		try {
			await api.post(`/users/${user.id}/totp/reset`, {});
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function setPassword(e: SubmitEvent) {
		e.preventDefault();
		try {
			await api.post(`/users/${pwUser!.id}/password`, { password: newPassword });
			pwOpen = false;
			newPassword = '';
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-4">
	<PageHeader title={t('nav.users')}>
		{#snippet actions()}
			{#if can('users.manage') && tab === 'users'}<button class="btn btn-primary" onclick={create}
					>{t('users.new')}</button
				>{/if}
		{/snippet}
	</PageHeader>
	<Tabs
		label={t('nav.users')}
		tabs={[
			{ id: 'users', label: t('users.tabUsers') },
			{ id: 'roles', label: t('users.tabRoles') }
		]}
		bind:value={tab}
		onchange={(id) => history.replaceState(history.state, '', id === 'roles' ? '#roles' : '#')}
	/>
	{#if tab === 'roles'}
		<RoleEditor onchange={load} />
	{:else}
		<ErrorBox {error} />
		<div class="card scroll-x">
			<table class="table">
				<thead>
					<tr>
						<th>{t('users.username')}</th>
						<th>{t('users.displayName')}</th>
						<th class="col-opt">{t('users.role')}</th>
						<th class="col-opt-lg">{t('users.lastLogin')}</th>
						<th></th>
					</tr>
				</thead>
				<tbody>
					{#each users as user (user.id)}
						<tr>
							<td class="font-mono">{user.username}</td>
							<td>
								{user.display_name}
								{#if !user.enabled}<span class="badge badge-muted">{t('common.disabled')}</span
									>{/if}
								{#if user.totp_enabled}<span class="badge badge-ok" title={t('mfa.title')}>2FA</span
									>{/if}
							</td>
							<td class="col-opt">
								{#if roleOf(user, roles)}{roleName(roleOf(user, roles)!)}{:else}{t(
										`roles.${user.role}`
									)}{/if}
							</td>
							<td class="col-opt-lg">{formatDateTime(user.last_login_at)}</td>
							<td class="space-x-1 text-right whitespace-nowrap">
								{#if can('users.manage')}
									<button class="btn btn-sm" onclick={() => edit(user)}>{t('common.edit')}</button>
									<button
										class="btn btn-sm"
										onclick={() => {
											pwUser = user;
											pwOpen = true;
										}}>{t('users.setPassword')}</button
									>
									{#if user.totp_enabled}
										<button class="btn btn-sm" onclick={() => resetTotp(user)}
											>{t('mfa.reset')}</button
										>
									{/if}
									<button class="btn btn-sm btn-danger" onclick={() => remove(user)}
										>{t('common.delete')}</button
									>
								{/if}
							</td>
						</tr>
					{:else}
						{#if !net.settled}<SkeletonRows cols={5} />{:else}<EmptyRow cols={5} />{/if}
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

<Modal title={editing.id ? t('common.edit') : t('users.new')} bind:open>
	<form class="space-y-3" onsubmit={save}>
		<ErrorBox {error} />
		{#if !editing.id}
			<div>
				<label for="u-username">{t('users.username')}</label>
				<input id="u-username" class="input" bind:value={editing.username} required />
			</div>
		{/if}
		<div>
			<label for="u-name">{t('users.displayName')}</label>
			<input id="u-name" class="input" bind:value={editing.display_name} required />
		</div>
		<div>
			<label for="u-email">{t('users.email')} ({t('common.optional')})</label>
			<input id="u-email" class="input" type="email" bind:value={editing.email} />
		</div>
		<div>
			<label for="u-role">{t('users.role')}</label>
			<select id="u-role" class="input" bind:value={editing.role_choice}>
				{#each choices as role (role.id)}<option value={role.id}>{roleName(role)}</option>{/each}
			</select>
			{#if roles.find((r) => r.id === editing.role_choice)}
				<p class="hint">{roleDescription(roles.find((r) => r.id === editing.role_choice)!)}</p>
			{/if}
		</div>
		{#if editing.id}
			<label class="flex items-center gap-2"
				><input type="checkbox" bind:checked={editing.enabled} /> {t('common.enabled')}</label
			>
		{:else}
			<div>
				<label for="u-pw">{t('users.password')}</label>
				<input
					id="u-pw"
					class="input"
					type="password"
					minlength="10"
					bind:value={editing.password}
				/>
				<p class="hint">{t('setup.passwordHint')}</p>
			</div>
		{/if}
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (open = false)}>{t('common.cancel')}</button>
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
</Modal>

<Modal title={t('users.setPassword')} bind:open={pwOpen}>
	<form class="space-y-3" onsubmit={setPassword}>
		<p class="text-sm">{pwUser?.display_name}</p>
		<input class="input" type="password" minlength="10" bind:value={newPassword} required />
		<p class="hint">{t('setup.passwordHint')}</p>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (pwOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
</Modal>
