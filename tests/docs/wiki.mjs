import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { buildWiki, headingIds, linkDestinations, writeWiki } from '../../scripts/docs/build-wiki.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const revision = 'a'.repeat(40);

function fixture(run) {
	const directory = mkdtempSync(path.join(os.tmpdir(), 'talkops-wiki-'));
	const manifest = {
		sections: [{ id: 'setup', en: 'Setup', de: 'Einrichtung' }],
		pages: [
			{ id: 'index', section: null, en: { source: 'docs/en/index.md', title: 'Handbook', slug: 'EN-Handbook' }, de: { source: 'docs/de/index.md', title: 'Handbuch', slug: 'DE-Handbuch' } },
			{ id: 'topic', section: 'setup', en: { source: 'docs/en/topic.md', title: 'Topic', slug: 'EN-Topic' }, de: { source: 'docs/de/thema.md', title: 'Thema', slug: 'DE-Thema' } }
		]
	};
	const put = (name, data) => {
		mkdirSync(path.dirname(path.join(directory, name)), { recursive: true });
		writeFileSync(path.join(directory, name), data);
	};
	put('docs/en/index.md', '# Handbook\n\n## Setup\n\n[Topic](topic.md)\n');
	put('docs/de/index.md', '# Handbuch\n\n## Einrichtung\n\n[Thema](thema.md)\n');
	put('docs/en/topic.md', '# Topic\n\n## Audio\n\n[Deutsch](../de/thema.md#töne)\n');
	put('docs/de/thema.md', '# Thema\n\n## Töne\n');
	const save = () => put('docs/wiki/pages.json', JSON.stringify(manifest));
	save();
	const build = () => buildWiki({ root: directory, revision });
	try { run({ directory, manifest, put, save, build }); }
	finally {
		const relative = path.relative(path.resolve(os.tmpdir()), path.resolve(directory));
		assert.ok(relative.startsWith('talkops-wiki-') && !relative.includes(path.sep), 'Cleanup must stay inside its own temporary fixture.');
		rmSync(directory, { recursive: true, force: true });
	}
}

test('rewrites paired pages and Unicode fragments with source attribution', () => fixture(({ build }) => {
	const { output, pageCount } = build();
	assert.equal(pageCount, 7);
	assert.match(output.get('EN-Topic.md').toString(), /\[Deutsch\]\(DE-Thema#töne\)/);
	assert.match(output.get('EN-Topic.md').toString(), new RegExp(`/blob/${revision}/docs/en/topic.md`));
	assert.match(output.get('EN-Topic.md').toString(), /\[Handbook\]\(EN-Handbook\)/);
	assert.match(output.get('_Sidebar.md').toString(), /DE-Handbuch#einrichtung/);
	assert.match(output.get('Home.md').toString(), /\(DE-Handbuch\)/);
}));

test('preserves external URLs, code examples and link titles', () => fixture(({ put, build }) => {
	put('docs/en/topic.md', '# Topic\n\n[PDF](https://example.com/(manual).pdf "Manual")\n\n[Target](topic.md "Title")\n\n`[sample](missing.md)`\n\n~~~md\n[x](missing.md)\n~~~\n');
	const text = build().output.get('EN-Topic.md').toString();
	assert.match(text, /https:\/\/example.com\/\(manual\).pdf "Manual"/);
	assert.match(text, /\[Target\]\(EN-Topic "Title"\)/);
	assert.match(text, /`\[sample\]\(missing.md\)`/);
	assert.match(text, /~~~md\n\[x\]\(missing.md\)\n~~~/);
}));

test('rewrites reference definitions and commit-pins source-only files and directories', () => fixture(({ put, build }) => {
	put('docs/adr/decision.md', '# Decision\n\n## Rationale\n');
	put('docs/en/topic.md', '# Topic\n\n[Decision][adr]\n\n[adr]: ../adr/decision.md#rationale "Design"\n\n[ADRs](../adr)\n');
	const text = build().output.get('EN-Topic.md').toString();
	assert.match(text, new RegExp(`/blob/${revision}/docs/adr/decision.md#rationale "Design"`));
	assert.match(text, new RegExp(`/tree/${revision}/docs/adr`));
}));

test('copies local images without basename collisions and preserves remote images', () => fixture(({ put, build }) => {
	put('docs/en/images/phone.png', Buffer.from([1, 2, 3]));
	put('docs/de/images/phone.png', Buffer.from([4, 5, 6]));
	put('docs/en/topic.md', '# Topic\n\n![A](images/phone.png)\n![B](../de/images/phone.png)\n![C](https://example.com/phone.png)\n');
	const { output } = build();
	const images = [...output.keys()].filter((name) => name.startsWith('assets/'));
	assert.equal(images.length, 2);
	assert.notEqual(images[0], images[1]);
	assert.deepEqual([...output.get(images[0])], [4, 5, 6]);
	assert.match(output.get('EN-Topic.md').toString(), /https:\/\/example.com\/phone.png/);
}));

test('rejects missing translation, duplicate slug and duplicate topic IDs', () => {
	fixture(({ manifest, save, build }) => { delete manifest.pages[1].de; save(); assert.throws(build, /translation pair/); });
	fixture(({ manifest, save, build }) => { manifest.pages[1].de.slug = 'en-topic'; save(); assert.throws(build, /Duplicate page/); });
	fixture(({ manifest, save, build }) => { manifest.pages[1].id = 'index'; save(); assert.throws(build, /duplicate topic/); });
});

test('rejects a missing target, missing fragment and unmapped guide', () => {
	fixture(({ put, build }) => { put('docs/en/topic.md', '# Topic\n\n[x](lost.md)'); assert.throws(build, /Missing local target/); });
	fixture(({ put, build }) => { put('docs/en/topic.md', '# Topic\n\n[x](topic.md#lost)'); assert.throws(build, /Missing anchor/); });
	fixture(({ put, build }) => { put('docs/de/extra.md', '# Extra'); assert.throws(build, /Unmapped guide/); });
});

test('rejects path escape, unsafe URL and invalid source path', () => {
	fixture(({ put, build }) => { put('docs/en/topic.md', '# Topic\n\n[x](../../../private.txt)'); assert.throws(build, /outside repository/); });
	fixture(({ put, build }) => { put('docs/en/topic.md', '# Topic\n\n[x](javascript:alert(1))'); assert.throws(build, /Unsupported URL/); });
	fixture(({ manifest, save, build }) => { manifest.pages[1].en.source = 'docs/en/../../secret.md'; save(); assert.throws(build, /translation pair/); });
});

test('uses GitHub-style heading IDs, including code and repeated headings', () => {
	assert.deepEqual([...headingIds('# Test\n## `One` & Two\n## Töne\n## Töne\n## Töne-1\n')], ['test', 'one--two', 'töne', 'töne-1', 'töne-1-1']);
	assert.equal(linkDestinations('`[x](bad)`\n```md\n[x](bad)\n```\n[x](<file with space.md>)').length, 1);
});

test('repeated builds are identical and marked working-copy previews are explicit', () => fixture(({ directory, build }) => {
	const one = build().output;
	const two = build().output;
	assert.deepEqual(one, two);
	const preview = buildWiki({ root: directory, revision, workingCopy: true });
	assert.match(preview.output.get('EN-Topic.md').toString(), /Working copy based on/);
}));

test('writes only to a dedicated output and preserves unmanaged existing files', () => fixture(({ directory, put, build }) => {
	const outputDir = path.join(directory, 'output');
	const { output } = build();
	writeWiki(output, outputDir);
	assert.deepEqual(readFileSync(path.join(outputDir, 'Home.md')), output.get('Home.md'));
	put('output/existing.md', 'Keep this page.');
	assert.throws(() => writeWiki(output, outputDir), /unmanaged file/);
	assert.equal(readFileSync(path.join(outputDir, 'existing.md'), 'utf8'), 'Keep this page.');
}));

test('refuses source and output directory links without writing through them', () => fixture(({ directory, put, build }) => {
	put('private/image.png', Buffer.from([7, 8, 9]));
	symlinkSync(path.join(directory, 'private'), path.join(directory, 'docs/en/images'), 'junction');
	put('docs/en/topic.md', '# Topic\n\n![Private](images/image.png)\n');
	assert.throws(build, /Symlink target/);
	const protectedDir = path.join(directory, 'private');
	symlinkSync(protectedDir, path.join(directory, 'output'), 'junction');
	assert.throws(() => writeWiki(new Map([['Home.md', Buffer.from('Do not write')]]), path.join(directory, 'output')), /Output symlink/);
	assert.deepEqual([...readFileSync(path.join(protectedDir, 'image.png'))], [7, 8, 9]);
}));

test('repository manifest covers 34 topic pairs and all UI page routes', () => {
	const manifest = JSON.parse(readFileSync(path.join(root, 'docs/wiki/pages.json'), 'utf8'));
	assert.equal(manifest.pages.length, 35);
	const covered = new Set(manifest.pages.flatMap((entry) => entry.routes ?? []));
	const exceptions = manifest.routeExceptions;
	const walk = (directory) => readdirSync(directory, { withFileTypes: true }).flatMap((entry) => entry.isDirectory() ? walk(path.join(directory, entry.name)) : [path.join(directory, entry.name)]);
	const routes = walk(path.join(root, 'web/src/routes')).filter((file) => path.basename(file) === '+page.svelte')
		.map((file) => '/' + path.relative(path.join(root, 'web/src/routes'), path.dirname(file)).split(path.sep).join('/'));
	for (const route of routes) assert.ok(covered.has(route) || exceptions[route], `No handbook topic for UI route ${route}`);
	for (const route of [...covered, ...Object.keys(exceptions)]) assert.ok(routes.includes(route), `Stale handbook route: ${route}`);
	const { output, pageCount } = buildWiki({ root, revision, workingCopy: true });
	assert.equal(pageCount, 73);
	for (const lang of ['en', 'de']) {
		const index = readFileSync(path.join(root, `docs/${lang}/index.md`), 'utf8');
		for (const entry of manifest.pages.filter((page) => page.id !== 'index')) {
			assert.ok(index.includes(`(${path.basename(entry[lang].source)})`), `Missing index topic ${entry.id}`);
		}
	}
	// Check generated navigation too, not only source links.
	for (const [name, data] of output) {
		if (!name.endsWith('.md')) continue;
		for (const link of linkDestinations(data.toString())) {
			if (/^https?:/.test(link.target)) continue;
			const [file, fragment] = link.target.split('#');
			const target = file ? `${file}.md` : name;
			if (file.startsWith('assets/')) { assert.ok(output.has(decodeURIComponent(file))); continue; }
			assert.ok(output.has(target), `Missing generated page from ${name}: ${link.target}`);
			if (fragment) assert.ok(headingIds(output.get(target).toString()).has(decodeURIComponent(fragment)), `Missing generated anchor: ${link.target}`);
		}
	}
});

test('README and maintainer links resolve with valid fragments', () => {
	for (const source of ['README.md', 'README.de.md', 'docs/wiki/README.md', 'docs/proposals/readme-wiki-de-en.md']) {
		for (const link of linkDestinations(readFileSync(path.join(root, source), 'utf8'))) {
			if (/^(?:https?:|mailto:)/.test(link.target)) continue;
			const [file, fragment] = link.target.split('#');
			const target = path.resolve(root, path.dirname(source), decodeURIComponent(file || path.basename(source)));
			const text = readFileSync(target, 'utf8');
			if (fragment) assert.ok(headingIds(text).has(decodeURIComponent(fragment)), `Missing anchor in ${source}: ${link.target}`);
		}
	}
});
