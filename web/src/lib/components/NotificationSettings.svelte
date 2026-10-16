<script lang="ts">
	import { onMount } from 'svelte';
	import {
		api,
		type NotificationEventType,
		type NotificationLog,
		type NotificationRule,
		type NotificationSeverity,
		type NotificationsView,
		type NotificationTestResult,
		type RoleRecord,
		type RolesOverview,
		type User
	} from '#lib/api.ts';
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import { formatDateTime, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { roleName } from '#lib/roles.ts';
	import { errorMessage } from '#lib/util.ts';

	type Draft = {
		enabled: boolean;
		min_severity: NotificationSeverity;
		recipient_users: string[];
		recipient_roles: string[];
		emails: string;
		delay_secs: number;
		send_recovery: boolean;
		digest_secs: number;
		rate_limit_per_hour: number;
	};

	let view = $state<NotificationsView | null>(null);
	let roles = $state<RoleRecord[]>([]);
	let users = $state<User[]>([]);
	let log = $state<NotificationLog | null>(null);
	let logTab = $state<'deliveries' | 'events'>('deliveries');
	let open = $state<string | null>(null);
	let draft = $state<Draft | null>(null);
	let error = $state('');
	let info = $state('');
	let busy = $state(false);

	/** Translation of a dynamic key; the key itself if there is none. */
	function tr(key: string): string {
		return t(key as MessageKey) ?? key;
	}
	const eventLabel = (key: string) => tr(`notifEvent.${key.replaceAll('.', '_')}`);

	async function load() {
		try {
			view = await api.get<NotificationsView>('/settings/notifications');
		} catch (err) {
			error = errorMessage(err);
			return;
		}
		// Recipient pickers need other permissions; without them they stay empty.
		const [r, u] = await Promise.allSettled([
			api.get<RolesOverview>('/roles'),
			api.get<User[]>('/users')
		]);
		if (r.status === 'fulfilled') roles = r.value.roles;
		if (u.status === 'fulfilled') users = u.value;
		await loadLog();
	}
	onMount(load);

	async function loadLog() {
		try {
			log = await api.get<NotificationLog>('/settings/notifications/log');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function rule(key: string): NotificationRule | undefined {
		return view?.rules.find((r) => r.event_type === key);
	}

	function toDraft(r: NotificationRule): Draft {
		return {
			enabled: r.enabled,
			min_severity: r.min_severity,
			recipient_users: [...r.recipient_users],
			recipient_roles: [...r.recipient_roles],
			emails: r.recipient_emails.join('\n'),
			delay_secs: r.delay_secs,
			send_recovery: r.send_recovery,
			digest_secs: r.digest_secs,
			rate_limit_per_hour: r.rate_limit_per_hour
		};
	}

	function body(d: Draft) {
		return {
			enabled: d.enabled,
			min_severity: d.min_severity,
			recipient_users: d.recipient_users,
			recipient_roles: d.recipient_roles,
			recipient_emails: d.emails
				.split(/[\s,;]+/)
				.map((e) => e.trim())
				.filter(Boolean),
			delay_secs: Number(d.delay_secs) || 0,
			send_recovery: d.send_recovery,
			digest_secs: Number(d.digest_secs) || 0,
			rate_limit_per_hour: Number(d.rate_limit_per_hour) || 1
		};
	}

	function replace(r: NotificationRule) {
		if (!view) return;
		view.rules = view.rules.map((x) => (x.event_type === r.event_type ? r : x));
	}

	function toggleOpen(key: string) {
		error = '';
		info = '';
		if (open === key) {
			open = null;
			draft = null;
			return;
		}
		const r = rule(key);
		open = key;
		draft = r ? toDraft(r) : null;
	}

	async function quickToggle(key: string, enabled: boolean) {
		const r = rule(key);
		if (!r) return;
		error = '';
		try {
			replace(
				await api.put<NotificationRule>(
					`/settings/notifications/rules/${key}`,
					body({ ...toDraft(r), enabled })
				)
			);
			if (open === key && draft) draft.enabled = enabled;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!open || !draft) return;
		busy = true;
		error = '';
		info = '';
		try {
			const r = await api.put<NotificationRule>(
				`/settings/notifications/rules/${open}`,
				body(draft)
			);
			replace(r);
			draft = toDraft(r);
			info = t('common.saved');
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	async function reset() {
		if (!open) return;
		busy = true;
		error = '';
		info = '';
		try {
			const r = await api.delJson<NotificationRule>(`/settings/notifications/rules/${open}`);
			replace(r);
			draft = toDraft(r);
			info = t('notif.resetDone');
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	async function test() {
		if (!open) return;
		busy = true;
		error = '';
		info = '';
		try {
			const res = await api.post<NotificationTestResult>(
				`/settings/notifications/rules/${open}/test`
			);
			if (res.status === 'sent') {
				info = t('notif.testSent', { to: res.recipients.join(', ') });
			} else {
				error = `${tr(`notif.status.${res.status}`)}${res.error ? `: ${res.error}` : ''}`;
			}
			await loadLog();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	function toggleIn(list: string[], id: string, on: boolean): string[] {
		return on ? [...new Set([...list, id])] : list.filter((x) => x !== id);
	}

	function summary(r: NotificationRule): string {
		const parts: string[] = [];
		const n = r.recipient_users.length + r.recipient_roles.length + r.recipient_emails.length;
		parts.push(n ? t('notif.recipientCount', { n }) : t('notif.noRecipients'));
		if (r.delay_secs) parts.push(t('notif.delayShort', { n: Math.round(r.delay_secs / 60) || 1 }));
		if (r.digest_secs)
			parts.push(t('notif.digestShort', { n: Math.round(r.digest_secs / 60) || 1 }));
		return parts.join(' · ');
	}

	const severityBadge: Record<string, string> = {
		info: 'badge-muted',
		warning: 'badge-warn',
		critical: 'badge-bad'
	};
	const statusBadge: Record<string, string> = {
		sent: 'badge-ok',
		failed: 'badge-bad',
		rate_limited: 'badge-warn',
		no_recipients: 'badge-warn',
		no_smtp: 'badge-warn'
	};

	const grouped = $derived(
		(view?.categories ?? []).map((c) => ({
			category: c,
			types: (view?.event_types ?? []).filter((e) => e.category === c)
		}))
	);
	const severities: NotificationSeverity[] = ['info', 'warning', 'critical'];
	const isProblem = (e: NotificationEventType) => !!e.recovery;
</script>

<div class="card space-y-4">
	<h2>{t('notif.title')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('notif.hint')}</p>
	<ErrorBox {error} />
	{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400">{info}</p>{/if}
	{#if view}
		{#each grouped as g (g.category)}
			<section class="space-y-1">
				<h3 class="font-medium">{tr(`notifCat.${g.category}`)}</h3>
				<ul class="divide-y divide-slate-100 dark:divide-slate-800">
					{#each g.types as e (e.key)}
						{@const r = rule(e.key)}
						<li class="py-2">
							<div class="flex flex-wrap items-center gap-x-3 gap-y-1">
								<label class="flex min-w-0 flex-1 items-center gap-2">
									<input
										type="checkbox"
										checked={r?.enabled ?? false}
										disabled={!e.implemented}
										onchange={(ev) => quickToggle(e.key, ev.currentTarget.checked)}
									/>
									<span class="min-w-0">
										<span class="block">{eventLabel(e.key)}</span>
										{#if r}<span class="block text-xs text-slate-500 dark:text-slate-400"
												>{summary(r)}</span
											>{/if}
									</span>
								</label>
								<span class="badge {severityBadge[e.severity]}"
									>{tr(`notif.sev.${e.severity}`)}</span
								>
								{#if !e.implemented}<span class="badge badge-muted">{t('notif.planned')}</span>{/if}
								<button
									class="btn btn-sm"
									type="button"
									aria-expanded={open === e.key}
									onclick={() => toggleOpen(e.key)}
									>{open === e.key ? t('common.close') : t('notif.edit')}</button
								>
							</div>
							{#if open === e.key && draft}
								<form
									class="mt-3 space-y-3 rounded-lg bg-slate-50 p-3 dark:bg-slate-800/50"
									onsubmit={save}
								>
									<label class="flex items-center gap-2 font-medium"
										><input type="checkbox" bind:checked={draft.enabled} />
										{t('notif.enabled')}</label
									>
									<div class="grid grid-cols-1 gap-3 @xl:grid-cols-2">
										<div>
											<label for="n-sev">{t('notif.minSeverity')}</label>
											<select id="n-sev" class="input" bind:value={draft.min_severity}>
												{#each severities as s (s)}
													<option value={s}>{tr(`notif.sev.${s}`)}</option>
												{/each}
											</select>
										</div>
										<div>
											<label for="n-rate">{t('notif.rateLimit')}</label>
											<input
												id="n-rate"
												class="input"
												type="number"
												min="1"
												max="1000"
												bind:value={draft.rate_limit_per_hour}
											/>
										</div>
										{#if isProblem(e)}
											<div>
												<label for="n-delay">{t('notif.delay')}</label>
												<input
													id="n-delay"
													class="input"
													type="number"
													min="0"
													max="86400"
													bind:value={draft.delay_secs}
												/>
												<p class="hint">{t('notif.delayHint')}</p>
											</div>
											<label class="flex items-center gap-2 @xl:mt-6"
												><input type="checkbox" bind:checked={draft.send_recovery} />
												{t('notif.sendRecovery')}</label
											>
										{:else}
											<div>
												<label for="n-digest">{t('notif.digest')}</label>
												<input
													id="n-digest"
													class="input"
													type="number"
													min="0"
													max="86400"
													bind:value={draft.digest_secs}
												/>
												<p class="hint">{t('notif.digestHint')}</p>
											</div>
										{/if}
									</div>
									<fieldset class="space-y-1">
										<legend class="font-medium">{t('notif.recipients')}</legend>
										{#if roles.length}
											<p class="text-xs text-slate-500 dark:text-slate-400">{t('notif.roles')}</p>
											<div class="flex flex-wrap gap-x-4 gap-y-1">
												{#each roles as role (role.id)}
													<label class="flex items-center gap-2"
														><input
															type="checkbox"
															checked={draft.recipient_roles.includes(role.id)}
															onchange={(ev) => {
																if (draft)
																	draft.recipient_roles = toggleIn(
																		draft.recipient_roles,
																		role.id,
																		ev.currentTarget.checked
																	);
															}}
														/>
														{roleName(role)}</label
													>
												{/each}
											</div>
										{/if}
										{#if users.length}
											<p class="text-xs text-slate-500 dark:text-slate-400">{t('notif.users')}</p>
											<div class="flex flex-wrap gap-x-4 gap-y-1">
												{#each users as u (u.id)}
													<label class="flex items-center gap-2"
														><input
															type="checkbox"
															checked={draft.recipient_users.includes(u.id)}
															onchange={(ev) => {
																if (draft)
																	draft.recipient_users = toggleIn(
																		draft.recipient_users,
																		u.id,
																		ev.currentTarget.checked
																	);
															}}
														/>
														{u.display_name}
														{#if !u.email}<span class="text-xs text-slate-500 dark:text-slate-400"
																>({t('notif.noEmail')})</span
															>{/if}</label
													>
												{/each}
											</div>
										{/if}
										<label for="n-emails">{t('notif.emails')}</label>
										<textarea
											id="n-emails"
											class="input font-mono"
											rows="2"
											placeholder="ops@example.com"
											bind:value={draft.emails}></textarea>
										<p class="hint">{t('notif.emailsHint')}</p>
									</fieldset>
									<div class="flex flex-wrap justify-end gap-2">
										<button class="btn" type="button" disabled={busy} onclick={reset}
											>{t('notif.reset')}</button
										>
										<button class="btn" type="button" disabled={busy} onclick={test}
											>{t('notif.test')}</button
										>
										<button class="btn btn-primary" type="submit" disabled={busy}
											>{t('common.save')}</button
										>
									</div>
								</form>
							{/if}
						</li>
					{/each}
				</ul>
			</section>
		{/each}
	{/if}
</div>

<div class="card space-y-3">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h2>{t('notif.log')}</h2>
		<button class="btn btn-sm" type="button" onclick={loadLog}>{t('notif.refresh')}</button>
	</div>
	<Tabs
		variant="pills"
		label={t('notif.log')}
		tabs={[
			{ id: 'deliveries', label: t('notif.deliveries') },
			{ id: 'events', label: t('notif.events') }
		]}
		bind:value={logTab}
	/>
	<div class="scroll-x">
		{#if logTab === 'deliveries'}
			<table class="w-full text-sm">
				<thead>
					<tr>
						<th class="text-left">{t('notif.time')}</th>
						<th class="text-left">{t('notif.subject')}</th>
						<th class="col-opt text-left">{t('notif.recipients')}</th>
						<th class="text-left">{t('notif.status')}</th>
					</tr>
				</thead>
				<tbody>
					{#each log?.deliveries ?? [] as d (d.id)}
						<tr>
							<td class="whitespace-nowrap">{formatDateTime(d.created_at)}</td>
							<td>
								<span class="block">{d.subject || eventLabel(d.event_type)}</span>
								<span class="block text-xs text-slate-500 dark:text-slate-400"
									>{tr(`notif.kind.${d.kind}`)}{d.event_count > 1
										? ` · ${d.event_count}`
										: ''}</span
								>
							</td>
							<td class="col-opt">{d.recipients.join(', ') || '—'}</td>
							<td>
								<span class="badge {statusBadge[d.status] ?? 'badge-muted'}"
									>{tr(`notif.status.${d.status}`)}</span
								>
								{#if d.error}<span class="block text-xs text-red-700 dark:text-red-400"
										>{d.error}</span
									>{/if}
							</td>
						</tr>
					{:else}
						<EmptyRow cols={4} text={t('notif.noDeliveries')} />
					{/each}
				</tbody>
			</table>
		{:else}
			<table class="w-full text-sm">
				<thead>
					<tr>
						<th class="text-left">{t('notif.time')}</th>
						<th class="text-left">{t('notif.event')}</th>
						<th class="col-opt text-left">{t('notif.source')}</th>
						<th class="text-left">{t('notif.severity')}</th>
					</tr>
				</thead>
				<tbody>
					{#each log?.events ?? [] as ev (ev.id)}
						<tr>
							<td class="whitespace-nowrap">{formatDateTime(ev.created_at)}</td>
							<td>{eventLabel(ev.event_type)}</td>
							<td class="col-opt font-mono text-xs">{ev.source || '—'}</td>
							<td
								><span class="badge {severityBadge[ev.severity]}"
									>{tr(`notif.sev.${ev.severity}`)}</span
								></td
							>
						</tr>
					{:else}
						<EmptyRow cols={4} text={t('notif.noEvents')} />
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>
