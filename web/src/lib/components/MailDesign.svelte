<script lang="ts">
	import { onMount } from 'svelte';
	import {
		api,
		upload,
		type MailBranding,
		type MailDesignView,
		type MailTemplateEntry,
		type RenderedMail
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Skeleton from '#lib/components/Skeleton.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import { i18n, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Lang = 'de' | 'en';
	type Draft = { subject: string; body: string };

	let view = $state<MailDesignView | null>(null);
	let branding = $state<MailBranding | null>(null);
	let error = $state('');
	let info = $state('');
	let logoVersion = $state(0);
	let logoInput = $state<HTMLInputElement>();

	let kind = $state('voicemail');
	let lang = $state<Lang>(i18n.locale === 'en' ? 'en' : 'de');
	let drafts = $state<Record<string, Draft>>({});
	let tplError = $state('');
	let tplInfo = $state('');
	let testTo = $state('');
	let preview = $state<RenderedMail | null>(null);
	let previewError = $state('');
	let previewMode = $state<'html' | 'text'>('html');
	let subjectEl = $state<HTMLInputElement>();
	let bodyEl = $state<HTMLTextAreaElement>();
	let lastField: 'subject' | 'body' = 'body';

	const key = (k: string, l: string) => `${k}/${l}`;
	const entry = $derived(
		view?.templates.find((x) => x.kind === kind && x.language === lang) ?? null
	);
	const draft = $derived(drafts[key(kind, lang)]);
	const dirty = $derived(
		!!entry && !!draft && (draft.subject !== entry.subject || draft.body !== entry.body)
	);
	const placeholders = $derived(view?.kinds.find((k) => k.key === kind)?.placeholders ?? []);

	function label(prefix: 'mailKind' | 'mailPh', name: string): string {
		const k = `${prefix}.${name.replaceAll('.', '_')}` as MessageKey;
		return t(k) ?? name;
	}

	function setTemplates(list: MailTemplateEntry[], keep: boolean) {
		if (!view) return;
		view.templates = list;
		for (const e of list) {
			const k = key(e.kind, e.language);
			if (!keep || !drafts[k]) drafts[k] = { subject: e.subject, body: e.body };
		}
	}

	onMount(async () => {
		try {
			view = await api.get<MailDesignView>('/settings/mail');
			branding = { ...view.branding };
			setTemplates(view.templates, false);
		} catch (err) {
			error = errorMessage(err);
		}
	});

	async function saveBranding(e: SubmitEvent) {
		e.preventDefault();
		if (!branding || !view) return;
		error = '';
		info = '';
		try {
			branding = await api.put<MailBranding>('/settings/mail/branding', branding);
			view.branding = { ...branding };
			view.instance.name = branding.instance_name;
			info = t('common.saved');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function chooseLogo(e: Event) {
		const file = (e.currentTarget as HTMLInputElement).files?.[0];
		if (!file || !branding) return;
		error = '';
		info = '';
		if (view && file.size > view.max_logo_bytes) {
			error = t('mailDesign.logoTooLarge', { kb: Math.round(view.max_logo_bytes / 1024) });
			return;
		}
		try {
			const form = new FormData();
			form.append('file', file);
			const b = await upload<MailBranding>('/settings/mail/logo', form);
			branding.has_logo = b.has_logo;
			branding.logo_width = b.logo_width;
			branding.logo_height = b.logo_height;
			logoVersion++;
		} catch (err) {
			error = errorMessage(err);
		} finally {
			if (logoInput) logoInput.value = '';
		}
	}

	async function removeLogo() {
		if (!branding) return;
		error = '';
		try {
			const b = await api.delJson<MailBranding>('/settings/mail/logo');
			branding.has_logo = b.has_logo;
			logoVersion++;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function insert(name: string) {
		if (!draft) return;
		const token = `{{ ${name} }}`;
		const el = lastField === 'subject' ? subjectEl : bodyEl;
		const value = draft[lastField];
		const start = el?.selectionStart ?? value.length;
		const end = el?.selectionEnd ?? value.length;
		draft[lastField] = value.slice(0, start) + token + value.slice(end);
		requestAnimationFrame(() => {
			el?.focus();
			el?.setSelectionRange(start + token.length, start + token.length);
		});
	}

	function body(extra: Record<string, unknown> = {}) {
		return {
			kind,
			language: lang,
			subject: draft?.subject ?? '',
			body: draft?.body ?? '',
			branding,
			...extra
		};
	}

	// Live preview with sample values, a short moment after the last change.
	let previewSeq = 0;
	$effect(() => {
		const payload = JSON.stringify(body());
		if (!view || !draft) return;
		const seq = ++previewSeq;
		const timer = setTimeout(async () => {
			try {
				const result = await api.post<RenderedMail>('/settings/mail/preview', JSON.parse(payload));
				if (seq !== previewSeq) return;
				preview = result;
				previewError = '';
			} catch (err) {
				if (seq === previewSeq) previewError = errorMessage(err);
			}
		}, 350);
		return () => clearTimeout(timer);
	});

	async function saveTemplate(e: SubmitEvent) {
		e.preventDefault();
		if (!draft) return;
		tplError = '';
		tplInfo = '';
		try {
			const list = await api.put<MailTemplateEntry[]>(`/settings/mail/templates/${kind}/${lang}`, {
				subject: draft.subject,
				body: draft.body
			});
			setTemplates(list, true);
			tplInfo = t('common.saved');
		} catch (err) {
			tplError = errorMessage(err);
		}
	}

	async function resetTemplate() {
		if (!entry || !confirm(t('mailDesign.resetConfirm'))) return;
		tplError = '';
		tplInfo = '';
		try {
			if (entry.custom) {
				const list = await api.delJson<MailTemplateEntry[]>(
					`/settings/mail/templates/${kind}/${lang}`
				);
				setTemplates(list, true);
			}
			drafts[key(kind, lang)] = { subject: entry.default_subject, body: entry.default_body };
		} catch (err) {
			tplError = errorMessage(err);
		}
	}

	async function sendTest() {
		tplError = '';
		tplInfo = '';
		try {
			await api.post('/settings/mail/test', body({ to: testTo }));
			tplInfo = t('smtp.testSent');
		} catch (err) {
			tplError = errorMessage(err);
		}
	}
</script>

{#if view && branding}
	<form class="card space-y-4" onsubmit={saveBranding}>
		<h2>{t('mailDesign.title')}</h2>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('mailDesign.hint')}</p>
		<ErrorBox {error} />
		{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400">{info}</p>{/if}
		<div class="grid grid-cols-1 gap-3 @xl:grid-cols-2">
			<div>
				<label for="md-instance">{t('mailDesign.instanceName')}</label>
				<input
					id="md-instance"
					class="input"
					maxlength="60"
					bind:value={branding.instance_name}
					placeholder={t('mailDesign.instanceNamePlaceholder')}
				/>
				<p class="hint">{t('mailDesign.instanceNameHint')}</p>
			</div>
			<div>
				<label for="md-sender">{t('mailDesign.senderName')}</label>
				<input
					id="md-sender"
					class="input"
					maxlength="80"
					bind:value={branding.sender_name}
					placeholder="TalkOps"
				/>
				<p class="hint">{t('mailDesign.senderNameHint')}</p>
			</div>
			<div>
				<label for="md-accent">{t('mailDesign.accent')}</label>
				<div class="flex items-center gap-2">
					<input
						type="color"
						class="h-9 w-12 shrink-0 cursor-pointer rounded-md border border-slate-300 bg-transparent p-0.5 dark:border-slate-700"
						aria-label={t('mailDesign.accent')}
						bind:value={branding.accent_color}
					/>
					<input
						id="md-accent"
						class="input font-mono"
						maxlength="7"
						bind:value={branding.accent_color}
					/>
				</div>
			</div>
			<div>
				<span class="mb-1 block text-sm font-medium">{t('mailDesign.logo')}</span>
				<div class="flex flex-wrap items-center gap-2">
					{#if branding.has_logo}
						<img
							src="/api/v1/settings/mail/logo?v={logoVersion}"
							alt={t('mailDesign.logo')}
							class="h-10 max-w-40 rounded-md border border-slate-200 bg-white object-contain p-1 dark:border-slate-700"
						/>
					{/if}
					<label class="btn cursor-pointer">
						{branding.has_logo ? t('mailDesign.logoReplace') : t('mailDesign.logoUpload')}
						<input
							bind:this={logoInput}
							type="file"
							accept="image/png,image/jpeg,image/gif"
							class="sr-only"
							onchange={chooseLogo}
						/>
					</label>
					{#if branding.has_logo}
						<button type="button" class="btn" onclick={removeLogo}>{t('common.delete')}</button>
					{/if}
				</div>
				<p class="hint">
					{t('mailDesign.logoHint', { kb: Math.round(view.max_logo_bytes / 1024) })}
				</p>
			</div>
		</div>
		<div>
			<label for="md-header">{t('mailDesign.header')}</label>
			<input id="md-header" class="input" maxlength="200" bind:value={branding.header_text} />
		</div>
		<div>
			<label for="md-footer">{t('mailDesign.footer')}</label>
			<textarea id="md-footer" class="input" rows="3" bind:value={branding.footer_text}></textarea>
			<p class="hint">
				{t('mailDesign.footerHint', {
					placeholders: view.common_placeholders.map((p) => `{{ ${p} }}`).join(', ')
				})}
			</p>
		</div>
		<dl
			class="grid grid-cols-1 gap-x-4 gap-y-1 rounded-lg bg-slate-50 p-3 text-sm @lg:grid-cols-[auto_1fr] dark:bg-slate-800/60"
		>
			<dt class="text-slate-500 dark:text-slate-400">{t('mailDesign.publicUrl')}</dt>
			<dd class="min-w-0 break-all">
				{view.instance.url || t('mailDesign.publicUrlMissing')}
			</dd>
			<dt class="text-slate-500 dark:text-slate-400">{t('mailDesign.hostname')}</dt>
			<dd class="font-mono break-all">{view.instance.hostname}</dd>
			<dt class="text-slate-500 dark:text-slate-400">{t('mailDesign.version')}</dt>
			<dd>TalkOps v{view.instance.version}</dd>
		</dl>
		<p class="hint">{t('mailDesign.poweredHint')}</p>
		<div class="flex justify-end">
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>

	<form class="card space-y-4" onsubmit={saveTemplate}>
		<h2>{t('mailDesign.templates')}</h2>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('mailDesign.templatesHint')}</p>
		<div class="flex flex-wrap items-center justify-between gap-2">
			<Tabs
				variant="pills"
				label={t('mailDesign.kind')}
				tabs={view.kinds.map((k) => ({ id: k.key, label: label('mailKind', k.key) }))}
				bind:value={kind}
			/>
			<Tabs
				variant="pills"
				label={t('mailDesign.language')}
				tabs={[
					{ id: 'de', label: 'Deutsch' },
					{ id: 'en', label: 'English' }
				]}
				bind:value={lang}
			/>
		</div>
		<ErrorBox error={tplError} />
		{#if tplInfo}<p class="text-sm text-emerald-700 dark:text-emerald-400">{tplInfo}</p>{/if}
		{#if draft && entry}
			<div class="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
				<div class="min-w-0 space-y-3">
					<div>
						<label for="md-subject">
							{t('mailDesign.subject')}
							{#if entry.custom}<span class="badge-ok ml-1">{t('mailDesign.custom')}</span>{/if}
						</label>
						<input
							id="md-subject"
							class="input"
							maxlength="300"
							bind:this={subjectEl}
							bind:value={draft.subject}
							onfocus={() => (lastField = 'subject')}
						/>
					</div>
					<div>
						<label for="md-body">{t('mailDesign.body')}</label>
						<textarea
							id="md-body"
							class="input font-mono text-sm"
							rows="14"
							bind:this={bodyEl}
							bind:value={draft.body}
							onfocus={() => (lastField = 'body')}></textarea>
						<p class="hint">{t('mailDesign.markdownHint')}</p>
					</div>
					<div>
						<span class="mb-1 block text-sm font-medium">{t('mailDesign.placeholders')}</span>
						<div class="flex flex-wrap gap-1">
							{#each placeholders as p (p)}
								<button
									type="button"
									class="rounded-md border border-slate-300 px-2 py-0.5 text-xs hover:bg-slate-100 dark:border-slate-700 dark:hover:bg-slate-800"
									title={`{{ ${p} }}`}
									onclick={() => insert(p)}>{label('mailPh', p)}</button
								>
							{/each}
						</div>
					</div>
				</div>
				<div class="min-w-0 space-y-2">
					<div class="flex flex-wrap items-center justify-between gap-2">
						<span class="text-sm font-medium">{t('mailDesign.preview')}</span>
						<Tabs
							variant="pills"
							label={t('mailDesign.preview')}
							tabs={[
								{ id: 'html', label: 'HTML' },
								{ id: 'text', label: t('mailDesign.plainText') }
							]}
							bind:value={previewMode}
						/>
					</div>
					<ErrorBox error={previewError} />
					{#if preview}
						<p class="truncate rounded-lg bg-slate-50 px-3 py-2 text-sm dark:bg-slate-800/60">
							<span class="text-slate-500 dark:text-slate-400">{t('mailDesign.subject')}:</span>
							{preview.subject}
						</p>
						{#if previewMode === 'html'}
							<iframe
								title={t('mailDesign.preview')}
								sandbox=""
								srcdoc={preview.html}
								class="h-[34rem] w-full rounded-lg border border-slate-200 bg-white dark:border-slate-700"
							></iframe>
						{:else}
							<pre
								class="h-[34rem] overflow-auto rounded-lg border border-slate-200 p-3 text-xs whitespace-pre-wrap dark:border-slate-700">{preview.text}</pre>
						{/if}
					{:else}
						<Skeleton lines={8} />
					{/if}
				</div>
			</div>
			<div class="flex flex-wrap items-end justify-between gap-2">
				<div class="flex flex-wrap items-end gap-2">
					<div>
						<label for="md-test">{t('smtp.testTo')}</label>
						<input id="md-test" class="input" type="email" bind:value={testTo} />
					</div>
					<button type="button" class="btn" disabled={!testTo} onclick={sendTest}
						>{t('mailDesign.sendTest')}</button
					>
				</div>
				<div class="flex gap-2">
					<button
						type="button"
						class="btn"
						disabled={!entry.custom &&
							draft.subject === entry.default_subject &&
							draft.body === entry.default_body}
						onclick={resetTemplate}>{t('mailDesign.reset')}</button
					>
					<button class="btn btn-primary" disabled={!dirty}>{t('common.save')}</button>
				</div>
			</div>
		{/if}
	</form>
{:else if error}
	<ErrorBox {error} />
{:else}
	<div class="card"><Skeleton lines={6} /></div>
{/if}
