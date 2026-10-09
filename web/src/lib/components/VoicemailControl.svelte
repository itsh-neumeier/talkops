<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import type { Voice } from '#lib/audio.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Keys = {
		listen: string;
		greeting: string;
		exit: string;
		repeat: string;
		delete: string;
		save: string;
		next: string;
	};
	type Config = {
		keys: Keys;
		texts: Record<string, Record<string, string>>;
		voices: Record<string, number>;
		announce: { name: boolean; number: boolean; date: boolean; cnam: boolean };
	};
	type Prompt = { key: string; placeholders: string[]; defaults: Record<string, string> };

	const LANGS = ['de', 'en'] as const;
	const KEYS = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '*', '#'];
	const MAIN: (keyof Keys)[] = ['listen', 'greeting', 'exit'];
	const MESSAGE: (keyof Keys)[] = ['repeat', 'delete', 'save', 'next'];

	let config = $state<Config | null>(null);
	let prompts = $state<Prompt[]>([]);
	let voices = $state<Voice[]>([]);
	let lang = $state<(typeof LANGS)[number]>('de');
	let error = $state('');
	let saved = $state(false);

	onMount(async () => {
		try {
			const v = await api.get<{ config: Config; prompts: Prompt[] }>('/settings/voicemail');
			config = v.config;
			prompts = v.prompts;
			voices = await api.get<Voice[]>('/audio/voices');
		} catch (err) {
			error = errorMessage(err);
		}
	});

	/** Keys used twice within a menu. */
	function duplicates(menu: (keyof Keys)[]): boolean {
		if (!config) return false;
		const used = menu.map((m) => config!.keys[m]);
		return new Set(used).size !== used.length;
	}

	function textOf(p: Prompt): string {
		return config?.texts[lang]?.[p.key] ?? p.defaults[lang] ?? '';
	}

	function setText(p: Prompt, value: string) {
		if (!config) return;
		const texts = { ...(config.texts[lang] ?? {}) };
		if (value.trim() === (p.defaults[lang] ?? '')) delete texts[p.key];
		else texts[p.key] = value;
		config.texts = { ...config.texts, [lang]: texts };
	}

	const changed = (p: Prompt) => config?.texts[lang]?.[p.key] !== undefined;

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!config) return;
		error = '';
		saved = false;
		try {
			const v = await api.put<{ config: Config; prompts: Prompt[] }>('/settings/voicemail', config);
			config = v.config;
			saved = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

{#if config}
	<form class="space-y-4" onsubmit={save}>
		<ErrorBox {error} />
		{#if saved}<p class="text-sm text-emerald-700 dark:text-emerald-400" data-testid="vmc-saved">
				{t('vmc.saved')}
			</p>{/if}

		<section class="card space-y-3">
			<h2>{t('vmc.keys')}</h2>
			<p class="text-sm text-slate-600 dark:text-slate-300">{t('vmc.keysHint')}</p>
			<div class="grid gap-4 md:grid-cols-2">
				{#each [{ title: 'vmc.mainMenu', items: MAIN }, { title: 'vmc.messageMenu', items: MESSAGE }] as menu (menu.title)}
					<div class="space-y-2">
						<h3 class="font-medium">{t(menu.title as MessageKey)}</h3>
						{#each menu.items as item (item)}
							<label class="flex items-center justify-between gap-3 text-sm"
								>{t(`vmc.key.${item}` as MessageKey)}
								<select
									class="input mt-0 w-24 font-mono"
									bind:value={config.keys[item]}
									data-testid="vmc-key-{item}"
								>
									{#each KEYS as k (k)}<option value={k}>{k}</option>{/each}
								</select></label
							>
						{/each}
						{#if duplicates(menu.items)}
							<p class="text-sm text-red-700 dark:text-red-400">{t('vmc.duplicate')}</p>
						{/if}
					</div>
				{/each}
			</div>
		</section>

		<section class="card space-y-3">
			<h2>{t('vmc.announce')}</h2>
			<p class="text-sm text-slate-600 dark:text-slate-300">{t('vmc.announceHint')}</p>
			{#each ['name', 'number', 'cnam', 'date'] as const as a (a)}
				<label class="flex items-center gap-2"
					><input type="checkbox" bind:checked={config.announce[a]} />
					{t(`vmc.announce.${a}` as MessageKey)}</label
				>
			{/each}
		</section>

		<section class="card space-y-3">
			<h2>{t('vmc.voices')}</h2>
			<p class="text-sm text-slate-600 dark:text-slate-300">{t('vmc.voicesHint')}</p>
			<div class="grid gap-3 sm:grid-cols-2">
				{#each LANGS as l (l)}
					<div>
						<label for="vmc-voice-{l}">{t(`vmc.lang.${l}` as MessageKey)}</label>
						<select
							id="vmc-voice-{l}"
							class="input"
							value={config.voices[l] ?? 1}
							onchange={(e) =>
								(config!.voices = {
									...config!.voices,
									[l]: Number((e.currentTarget as HTMLSelectElement).value)
								})}
						>
							{#each voices.filter((v) => v.language === l) as v (v.voice)}
								<option value={v.voice}
									>{v.name} ({t(v.gender === 'female' ? 'vmc.female' : 'vmc.male')})</option
								>
							{/each}
						</select>
					</div>
				{/each}
			</div>
		</section>

		<section class="card space-y-3">
			<div class="flex flex-wrap items-center justify-between gap-2">
				<h2>{t('vmc.texts')}</h2>
				<div class="flex gap-1" role="tablist">
					{#each LANGS as l (l)}
						<button
							type="button"
							role="tab"
							aria-selected={lang === l}
							class="btn btn-sm {lang === l ? 'btn-primary' : ''}"
							onclick={() => (lang = l)}>{t(`vmc.lang.${l}` as MessageKey)}</button
						>
					{/each}
				</div>
			</div>
			<p class="text-sm text-slate-600 dark:text-slate-300">{t('vmc.textsHint')}</p>
			{#each prompts as p (p.key)}
				<div>
					<div class="flex items-center justify-between gap-2">
						<label for="vmc-t-{p.key}" class="text-sm font-medium"
							>{t(`vmc.prompt.${p.key}` as MessageKey)}</label
						>
						{#if changed(p)}
							<button
								type="button"
								class="text-xs text-teal-700 hover:underline dark:text-teal-400"
								onclick={() => setText(p, p.defaults[lang] ?? '')}>{t('vmc.reset')}</button
							>
						{/if}
					</div>
					<textarea
						id="vmc-t-{p.key}"
						class="input min-h-0 text-sm {changed(p) ? 'border-teal-500' : ''}"
						rows="2"
						maxlength="500"
						value={textOf(p)}
						oninput={(e) => setText(p, (e.currentTarget as HTMLTextAreaElement).value)}></textarea>
					{#if p.placeholders.length}
						<p class="hint font-mono">{p.placeholders.map((x) => `{${x}}`).join(' ')}</p>
					{/if}
				</div>
			{/each}
		</section>

		<div class="flex justify-end">
			<button class="btn btn-primary" disabled={duplicates(MAIN) || duplicates(MESSAGE)}
				>{t('common.save')}</button
			>
		</div>
	</form>
{:else}
	<ErrorBox {error} />
{/if}
