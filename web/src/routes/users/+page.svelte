<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type Role, type User } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let users = $state<User[]>([]);
	let error = $state('');
	let editing = $state<Partial<User> & { password?: string }>({});
	let open = $state(false);
	let pwOpen = $state(false);
	let pwUser = $state<User | null>(null);
	let newPassword = $state('');
	const roles: Role[] = ['user', 'operator', 'admin'];

	async function load() {
		try {
			users = await api.get<User[]>('/users');
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
			enabled: true,
			password: ''
		};
		open = true;
	}

	function edit(user: User) {
		editing = { ...user };
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
					enabled: editing.enabled
				});
			} else {
				await api.post('/users', {
					username: editing.username,
					display_name: editing.display_name,
					email: editing.email,
					role: editing.role,
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
	<div class="flex items-center justify-between">
		<h1>{t('nav.users')}</h1>
		{#if hasRole('admin')}<button class="btn btn-primary" onclick={create}>{t('users.new')}</button
			>{/if}
	</div>
	<ErrorBox {error} />
	<div class="card overflow-x-auto">
		<table class="table">
			<thead>
				<tr>
					<th>{t('users.username')}</th>
					<th>{t('users.displayName')}</th>
					<th>{t('users.role')}</th>
					<th>{t('users.lastLogin')}</th>
					<th></th>
				</tr>
			</thead>
			<tbody>
				{#each users as user (user.id)}
					<tr>
						<td class="font-mono">{user.username}</td>
						<td>
							{user.display_name}
							{#if !user.enabled}<span class="badge badge-muted">{t('common.disabled')}</span>{/if}
						</td>
						<td>{t(`roles.${user.role}`)}</td>
						<td>{formatDateTime(user.last_login_at)}</td>
						<td class="space-x-1 text-right whitespace-nowrap">
							{#if hasRole('admin')}
								<button class="btn btn-sm" onclick={() => edit(user)}>{t('common.edit')}</button>
								<button
									class="btn btn-sm"
									onclick={() => {
										pwUser = user;
										pwOpen = true;
									}}>{t('users.setPassword')}</button
								>
								<button class="btn btn-sm btn-danger" onclick={() => remove(user)}
									>{t('common.delete')}</button
								>
							{/if}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
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
			<select id="u-role" class="input" bind:value={editing.role}>
				{#each roles as role (role)}<option value={role}>{t(`roles.${role}`)}</option>{/each}
			</select>
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
