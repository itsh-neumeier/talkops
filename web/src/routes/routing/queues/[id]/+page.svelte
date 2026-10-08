<script lang="ts">
	// Call queue settings like UniFi Talk: general, schedule, call handling.
	import { beforeNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { api, type Queue } from '#lib/api.ts';
	import AudioPicker, { type AudioMode } from '#lib/components/AudioPicker.svelte';
	import DestinationSelect from '#lib/components/DestinationSelect.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import ExtensionPicker from '#lib/components/ExtensionPicker.svelte';
	import MemberPicker from '#lib/components/routing/MemberPicker.svelte';
	import { loadTargets, targets } from '#lib/destinations.svelte.ts';
	import { i18n, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const strategies = [
		'longest-idle-agent',
		'ring-all',
		'round-robin',
		'top-down',
		'agent-with-fewest-calls',
		'random'
	];
	const tabs: { id: string; key: MessageKey }[] = [
		{ id: 'general', key: 'queues.tab.general' },
		{ id: 'schedule', key: 'queues.tab.schedule' },
		{ id: 'handling', key: 'queues.tab.handling' }
	];

	const id = $derived(page.params.id ?? 'new');
	type Form = Omit<Queue, 'id' | 'number'> & { number: string };
	let form = $state<Form | null>(null);
	let tab = $state('general');
	let error = $state('');
	let saved = $state(false);
	let saving = $state(false);
	let snapshot = $state('');
	// Not answered in time: forward to a destination or take a message.
	let unanswered = $state<'destination' | 'voicemail'>('destination');

	let greetingMode = $state<AudioMode>('none');
	let mohMode = $state<AudioMode>('default');
	let vmMode = $state<AudioMode>('default');
	let greetingPicker = $state<ReturnType<typeof AudioPicker>>();
	let mohPicker = $state<ReturnType<typeof AudioPicker>>();
	let vmPicker = $state<ReturnType<typeof AudioPicker>>();

	const current = () => JSON.stringify([form, unanswered]);
	const dirty = $derived(form !== null && current() !== snapshot);

	function blank(): Form {
		return {
			number: '',
			name: '',
			strategy: 'longest-idle-agent',
			max_wait_secs: 300,
			agent_timeout_secs: 20,
			wrap_up_secs: 5,
			timeout_type: 'none',
			timeout_id: null,
			enabled: true,
			members: [],
			greeting_clip_id: null,
			moh_clip_id: null,
			max_callers: 0,
			overflow_type: 'none',
			overflow_id: null,
			time_condition_id: null,
			closed_type: 'none',
			closed_id: null,
			voicemail_recipients: [],
			voicemail_clip_id: null
		};
	}

	onMount(async () => {
		try {
			await loadTargets();
			if (id === 'new') {
				form = blank();
			} else {
				const q = await api.get<Queue>(`/queues/${id}`);
				form = { ...q, number: q.number ?? '' };
				unanswered = q.voicemail_recipients.length ? 'voicemail' : 'destination';
				greetingMode = q.greeting_clip_id ? 'generate' : 'none';
				mohMode = q.moh_clip_id ? 'upload' : 'default';
				vmMode = q.voicemail_clip_id ? 'generate' : 'default';
				snapshot = current();
			}
		} catch (err) {
			error = errorMessage(err);
		}
	});

	beforeNavigate((nav) => {
		if (dirty && !saving && !confirm(t('flow.unsaved'))) nav.cancel();
	});

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!form) return;
		error = '';
		saved = false;
		saving = true;
		try {
			form.greeting_clip_id = (await greetingPicker?.ensure()) ?? null;
			form.moh_clip_id = (await mohPicker?.ensure()) ?? null;
			form.voicemail_clip_id =
				unanswered === 'voicemail' ? ((await vmPicker?.ensure()) ?? null) : null;
			const body = {
				...form,
				number: form.number || null,
				max_wait_secs: Number(form.max_wait_secs),
				agent_timeout_secs: Number(form.agent_timeout_secs),
				wrap_up_secs: Number(form.wrap_up_secs),
				max_callers: Number(form.max_callers),
				voicemail_recipients: unanswered === 'voicemail' ? form.voicemail_recipients : []
			};
			const q =
				id === 'new'
					? await api.post<Queue>('/queues', body)
					: await api.put<Queue>(`/queues/${id}`, body);
			snapshot = current();
			saved = true;
			if (id === 'new') await goto(`/routing/queues/${q.id}`, { replaceState: true });
		} catch (err) {
			error = errorMessage(err);
		} finally {
			saving = false;
		}
	}

	const language = $derived(i18n.locale === 'en' ? 'en' : 'de');
	const schedule = $derived(targets.conditions.find((c) => c.id === form?.time_condition_id));
</script>

<form class="space-y-4" onsubmit={save}>
	<div class="flex flex-wrap items-center justify-between gap-2">
		<div>
			<a href="/routing" class="text-sm text-teal-700 hover:underline dark:text-teal-300"
				>← {t('nav.routing')}</a
			>
			<h1>{form?.name || t('queues.new')}</h1>
		</div>
		{#if hasRole('admin')}
			<div class="flex items-center gap-3">
				{#if saved && !dirty}<span class="text-sm text-emerald-700 dark:text-emerald-400"
						>{t('common.saved')}</span
					>{:else if dirty}<span class="text-sm text-amber-600">{t('flow.dirty')}</span>{/if}
				<button class="btn btn-primary" disabled={saving || !form}>{t('common.save')}</button>
			</div>
		{/if}
	</div>
	<ErrorBox {error} />
	{#if form}
		<div
			class="flex flex-wrap gap-1 border-b border-slate-200 dark:border-slate-800"
			role="tablist"
		>
			{#each tabs as x (x.id)}
				<button
					type="button"
					role="tab"
					aria-selected={tab === x.id}
					class="rounded-t-md px-3 py-2 text-sm {tab === x.id
						? 'border-b-2 border-teal-600 font-medium text-teal-800 dark:text-teal-200'
						: 'text-slate-600 hover:text-slate-900 dark:text-slate-300'}"
					onclick={() => (tab = x.id)}>{t(x.key)}</button
				>
			{/each}
		</div>

		<!-- All tabs stay mounted (hidden) so pending audio is saved too. -->
		<section class="card space-y-4" class:hidden={tab !== 'general'}>
			<div class="grid gap-3 sm:grid-cols-[8rem_1fr]">
				<div>
					<label for="q-num">{t('routing.number')}</label>
					<input id="q-num" class="input font-mono" bind:value={form.number} placeholder="80" />
				</div>
				<div>
					<label for="q-name">{t('common.name')}</label>
					<input id="q-name" class="input" bind:value={form.name} required />
				</div>
			</div>
			<label class="flex items-center gap-2">
				<input type="checkbox" bind:checked={form.enabled} />
				{t('common.enabled')}
			</label>
			<div>
				<span class="text-sm font-medium">{t('queues.agents')}</span>
				<MemberPicker bind:members={form.members} />
				<p class="hint">{t('queues.agentsHint')}</p>
			</div>
		</section>

		<section class="card space-y-4" class:hidden={tab !== 'schedule'}>
			<div>
				<label for="q-tc">{t('queues.hours')}</label>
				<select id="q-tc" class="input" bind:value={form.time_condition_id}>
					<option value={null}>{t('queues.alwaysOpen')}</option>
					{#each targets.conditions as c (c.id)}
						<option value={c.id}>{c.number ? `${c.number} ` : ''}{c.name}</option>
					{/each}
				</select>
				<p class="hint">{t('flow.scheduleHint')}</p>
			</div>
			{#if schedule}
				<p class="text-sm">
					{t('flow.scheduleNow')}
					<span
						class={schedule.state.open ? 'text-emerald-700 dark:text-emerald-400' : 'text-red-600'}
						>{schedule.state.open ? t('flow.branch.open') : t('flow.branch.closed')}</span
					>
				</p>
				<div>
					<label for="q-closed">{t('queues.afterHours')}</label>
					<DestinationSelect
						inputId="q-closed"
						bind:type={form.closed_type}
						bind:id={form.closed_id}
						exclude={id}
						noneLabel={t('routing.hangup')}
					/>
				</div>
			{/if}
		</section>

		<section class="space-y-4" class:hidden={tab !== 'handling'}>
			<div class="card space-y-3">
				<h2>{t('queues.greeting')}</h2>
				<AudioPicker
					bind:this={greetingPicker}
					bind:mode={greetingMode}
					bind:clipId={form.greeting_clip_id}
					modes={['none', 'generate', 'record', 'upload']}
					{language}
					optional
					hints={{ none: t('queues.greetingNone') }}
				/>
			</div>
			<div class="card space-y-3">
				<h2>{t('queues.moh')}</h2>
				<AudioPicker
					bind:this={mohPicker}
					bind:mode={mohMode}
					bind:clipId={form.moh_clip_id}
					modes={['default', 'upload', 'record', 'generate']}
					{language}
					optional
					hints={{ default: t('queues.mohDefault'), upload: t('queues.mohHint') }}
				/>
			</div>
			<div class="card space-y-3">
				<h2>{t('queues.distribution')}</h2>
				<div class="grid gap-3 sm:grid-cols-3">
					<div>
						<label for="q-strat">{t('groups.strategy')}</label>
						<select id="q-strat" class="input" bind:value={form.strategy}>
							{#each strategies as s (s)}<option value={s}
									>{t(`queues.s.${s}` as MessageKey)}</option
								>{/each}
						</select>
					</div>
					<div>
						<label for="q-ring">{t('queues.agentTimeout')}</label>
						<input
							id="q-ring"
							class="input"
							type="number"
							min="5"
							max="120"
							bind:value={form.agent_timeout_secs}
						/>
					</div>
					<div>
						<label for="q-wrap">{t('queues.wrapUp')}</label>
						<input
							id="q-wrap"
							class="input"
							type="number"
							min="0"
							max="600"
							bind:value={form.wrap_up_secs}
						/>
					</div>
				</div>
			</div>
			<div class="card space-y-3">
				<h2>{t('queues.capacity')}</h2>
				<div class="grid gap-3 sm:grid-cols-2">
					<div>
						<label for="q-max">{t('queues.maxCallers')}</label>
						<input
							id="q-max"
							class="input"
							type="number"
							min="0"
							max="500"
							bind:value={form.max_callers}
						/>
					</div>
					{#if Number(form.max_callers) > 0}
						<div>
							<label for="q-over">{t('queues.overflow')}</label>
							<DestinationSelect
								inputId="q-over"
								bind:type={form.overflow_type}
								bind:id={form.overflow_id}
								exclude={id}
								noneLabel={t('routing.hangup')}
							/>
						</div>
					{/if}
				</div>
			</div>
			<div class="card space-y-3">
				<h2>{t('queues.unanswered')}</h2>
				<div>
					<label for="q-wait">{t('queues.maxWait')}</label>
					<input
						id="q-wait"
						class="input sm:w-48"
						type="number"
						min="0"
						max="7200"
						bind:value={form.max_wait_secs}
					/>
				</div>
				<div class="flex flex-wrap gap-4">
					<label class="flex items-center gap-2">
						<input type="radio" value="destination" bind:group={unanswered} />
						{t('queues.toDestination')}
					</label>
					<label class="flex items-center gap-2">
						<input type="radio" value="voicemail" bind:group={unanswered} />
						{t('queues.toVoicemail')}
					</label>
				</div>
				<div class:hidden={unanswered !== 'destination'}>
					<DestinationSelect
						inputId="q-fb"
						bind:type={form.timeout_type}
						bind:id={form.timeout_id}
						exclude={id}
						noneLabel={t('routing.hangup')}
					/>
				</div>
				<div class="space-y-3" class:hidden={unanswered !== 'voicemail'}>
					<ExtensionPicker bind:selected={form.voicemail_recipients} label={t('flow.recipients')} />
					<p class="hint">{t('flow.recipientsHint')}</p>
					<span class="text-sm font-medium">{t('vm.greeting')}</span>
					<AudioPicker
						bind:this={vmPicker}
						bind:mode={vmMode}
						bind:clipId={form.voicemail_clip_id}
						modes={['default', 'generate', 'record', 'upload']}
						{language}
						optional
						hints={{ default: t('flow.vmDefaultGreeting') }}
					/>
				</div>
			</div>
		</section>
	{/if}
</form>
