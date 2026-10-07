<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Settings = {
		public_url: string;
		oidc_enabled: boolean;
		oidc_issuer: string;
		oidc_client_id: string;
		oidc_has_secret: boolean;
		oidc_scopes: string;
		oidc_username_claim: string;
		oidc_groups_claim: string;
		oidc_button_label: string;
		ldap_enabled: boolean;
		ldap_url: string;
		ldap_starttls: boolean;
		ldap_bind_dn: string;
		ldap_has_password: boolean;
		ldap_base_dn: string;
		ldap_user_filter: string;
		ldap_username_attr: string;
		ldap_display_attr: string;
		ldap_email_attr: string;
		ldap_group_attr: string;
		admin_group: string;
		operator_group: string;
		user_group: string;
	};
	type TestResult = {
		dn: string | null;
		display_name: string | null;
		groups: string[];
		role: string | null;
	};

	let s = $state<Settings | null>(null);
	let secret = $state('');
	let bindPassword = $state('');
	let testUser = $state('');
	let testResult = $state<TestResult | null>(null);
	let error = $state('');
	let info = $state('');

	onMount(async () => {
		try {
			s = await api.get<Settings>('/settings/identity');
		} catch (err) {
			error = errorMessage(err);
		}
	});

	const callback = $derived(`${s?.public_url || location.origin}/api/v1/auth/oidc/callback`);

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		info = '';
		try {
			s = await api.put<Settings>('/settings/identity', {
				...s,
				oidc_client_secret: secret === '' ? null : secret,
				ldap_bind_password: bindPassword === '' ? null : bindPassword
			});
			secret = '';
			bindPassword = '';
			info = t('common.saved');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function test() {
		error = '';
		info = '';
		testResult = null;
		try {
			testResult = await api.post<TestResult>('/settings/identity/ldap-test', {
				username: testUser || null
			});
			if (!testUser) info = t('identity.ldapOk');
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

{#if s}
	<form class="card space-y-4" onsubmit={save}>
		<h2>{t('identity.title')}</h2>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('identity.hint')}</p>
		<ErrorBox {error} />
		{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400">{info}</p>{/if}

		<fieldset class="space-y-3">
			<legend class="font-medium">{t('identity.roles')}</legend>
			<p class="hint">{t('identity.rolesHint')}</p>
			<div class="grid gap-3 sm:grid-cols-3">
				<div>
					<label for="id-admin">{t('roles.admin')}</label>
					<input id="id-admin" class="input" bind:value={s.admin_group} placeholder="pbx-admins" />
				</div>
				<div>
					<label for="id-op">{t('roles.operator')}</label>
					<input id="id-op" class="input" bind:value={s.operator_group} />
				</div>
				<div>
					<label for="id-user">{t('roles.user')}</label>
					<input
						id="id-user"
						class="input"
						bind:value={s.user_group}
						placeholder={t('identity.everyone')}
					/>
				</div>
			</div>
		</fieldset>

		<fieldset class="space-y-3 border-t border-slate-100 pt-3 dark:border-slate-800">
			<legend class="sr-only">OIDC</legend>
			<label class="flex items-center gap-2 font-medium"
				><input type="checkbox" bind:checked={s.oidc_enabled} /> {t('identity.oidc')}</label
			>
			{#if s.oidc_enabled}
				<p class="hint">{t('identity.oidcHint')}</p>
				<div>
					<label for="id-iss">{t('identity.issuer')}</label>
					<input
						id="id-iss"
						class="input font-mono"
						bind:value={s.oidc_issuer}
						placeholder="https://auth.example.com/realms/home"
						required
					/>
				</div>
				<div class="grid gap-3 sm:grid-cols-2">
					<div>
						<label for="id-cid">{t('identity.clientId')}</label>
						<input id="id-cid" class="input font-mono" bind:value={s.oidc_client_id} required />
					</div>
					<div>
						<label for="id-sec">{t('identity.clientSecret')}</label>
						<input
							id="id-sec"
							class="input"
							type="password"
							autocomplete="new-password"
							bind:value={secret}
							placeholder={s.oidc_has_secret ? t('door.passwordKeep') : t('identity.publicClient')}
						/>
					</div>
				</div>
				<div>
					<span class="text-sm font-medium">{t('identity.redirectUri')}</span>
					<code class="block rounded bg-slate-100 p-2 text-sm break-all dark:bg-slate-800"
						>{callback}</code
					>
				</div>
				<details class="text-sm">
					<summary class="cursor-pointer">{t('identity.advanced')}</summary>
					<div class="mt-2 grid gap-3 sm:grid-cols-2">
						<div>
							<label for="id-scopes">{t('identity.scopes')}</label>
							<input id="id-scopes" class="input font-mono" bind:value={s.oidc_scopes} />
						</div>
						<div>
							<label for="id-label">{t('identity.buttonLabel')}</label>
							<input id="id-label" class="input" bind:value={s.oidc_button_label} />
						</div>
						<div>
							<label for="id-uc">{t('identity.usernameClaim')}</label>
							<input id="id-uc" class="input font-mono" bind:value={s.oidc_username_claim} />
						</div>
						<div>
							<label for="id-gc">{t('identity.groupsClaim')}</label>
							<input id="id-gc" class="input font-mono" bind:value={s.oidc_groups_claim} />
						</div>
						<div class="sm:col-span-2">
							<label for="id-pub">{t('identity.publicUrl')}</label>
							<input
								id="id-pub"
								class="input font-mono"
								bind:value={s.public_url}
								placeholder="https://pbx.example.com"
							/>
						</div>
					</div>
				</details>
			{/if}
		</fieldset>

		<fieldset class="space-y-3 border-t border-slate-100 pt-3 dark:border-slate-800">
			<legend class="sr-only">LDAP</legend>
			<label class="flex items-center gap-2 font-medium"
				><input type="checkbox" bind:checked={s.ldap_enabled} /> {t('identity.ldap')}</label
			>
			{#if s.ldap_enabled}
				<p class="hint">{t('identity.ldapHint')}</p>
				<div class="grid gap-3 sm:grid-cols-3">
					<div class="sm:col-span-2">
						<label for="id-lurl">URL</label>
						<input
							id="id-lurl"
							class="input font-mono"
							bind:value={s.ldap_url}
							placeholder="ldaps://dc1.example.com:636"
							required
						/>
					</div>
					<label class="mt-6 flex items-center gap-2"
						><input type="checkbox" bind:checked={s.ldap_starttls} /> StartTLS</label
					>
				</div>
				<div class="grid gap-3 sm:grid-cols-2">
					<div>
						<label for="id-bdn">{t('identity.bindDn')}</label>
						<input
							id="id-bdn"
							class="input font-mono"
							bind:value={s.ldap_bind_dn}
							placeholder="CN=svc-talkops,OU=Service,DC=example,DC=com"
						/>
					</div>
					<div>
						<label for="id-bpw">{t('door.password')}</label>
						<input
							id="id-bpw"
							class="input"
							type="password"
							autocomplete="new-password"
							bind:value={bindPassword}
							placeholder={s.ldap_has_password ? t('door.passwordKeep') : ''}
						/>
					</div>
				</div>
				<div>
					<label for="id-base">{t('identity.baseDn')}</label>
					<input
						id="id-base"
						class="input font-mono"
						bind:value={s.ldap_base_dn}
						placeholder="DC=example,DC=com"
						required
					/>
				</div>
				<div>
					<label for="id-filter">{t('identity.filter')}</label>
					<input id="id-filter" class="input font-mono" bind:value={s.ldap_user_filter} />
					<p class="hint">{t('identity.filterHint')}</p>
				</div>
				<div class="grid gap-3 sm:grid-cols-4">
					<div>
						<label for="id-ua">{t('identity.attrUser')}</label>
						<input id="id-ua" class="input font-mono" bind:value={s.ldap_username_attr} />
					</div>
					<div>
						<label for="id-da">{t('identity.attrName')}</label>
						<input id="id-da" class="input font-mono" bind:value={s.ldap_display_attr} />
					</div>
					<div>
						<label for="id-ma">{t('identity.attrMail')}</label>
						<input id="id-ma" class="input font-mono" bind:value={s.ldap_email_attr} />
					</div>
					<div>
						<label for="id-ga">{t('identity.attrGroups')}</label>
						<input id="id-ga" class="input font-mono" bind:value={s.ldap_group_attr} />
					</div>
				</div>
				<div class="flex flex-wrap items-end gap-2">
					<div>
						<label for="id-tu">{t('identity.testUser')}</label>
						<input id="id-tu" class="input" bind:value={testUser} />
					</div>
					<button type="button" class="btn" onclick={test}>{t('identity.test')}</button>
				</div>
				<p class="hint">{t('identity.testHint')}</p>
				{#if testResult?.dn}
					<div class="rounded-md bg-slate-50 p-3 text-sm dark:bg-slate-800/60">
						<div class="font-mono break-all">{testResult.dn}</div>
						<div>{testResult.display_name}</div>
						<div>
							{t('users.role')}:
							{testResult.role
								? t(`roles.${testResult.role}` as 'roles.user')
								: t('identity.noAccess')}
						</div>
						{#if testResult.groups.length}<div class="text-xs text-slate-500">
								{testResult.groups.join(' · ')}
							</div>{/if}
					</div>
				{/if}
			{/if}
		</fieldset>
		<div class="flex justify-end">
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
{/if}
