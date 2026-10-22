import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const posix = (value) => value.split(path.sep).join('/');
const encodePath = (value) => value.split('/').map(encodeURIComponent).join('/');

// Exclude code examples from link and heading processing. Keep line positions.
export function proseMask(markdown) {
	let fence = null;
	return markdown.split('\n').map((line) => {
		const match = line.match(/^\s{0,3}(`{3,}|~{3,})/);
		if (match && !fence) {
			fence = match[1];
			return ' '.repeat(line.length);
		}
		if (fence) {
			if (match && match[1][0] === fence[0] && match[1].length >= fence.length) fence = null;
			return ' '.repeat(line.length);
		}
		return line.replace(/(`+)(.*?)\1/g, (value) => ' '.repeat(value.length));
	}).join('\n');
}

export function headingIds(markdown) {
	const ids = new Set();
	const counts = new Map();
	let fence = null;
	for (const line of markdown.split('\n')) {
		const marker = line.match(/^\s{0,3}(`{3,}|~{3,})/);
		if (marker && !fence) { fence = marker[1]; continue; }
		if (fence) {
			if (marker && marker[1][0] === fence[0] && marker[1].length >= fence.length) fence = null;
			continue;
		}
		const heading = line.match(/^ {0,3}#{1,6}\s+(.+?)(?:\s+#+)?\s*$/);
		if (!heading) continue;
		const base = heading[1].replace(/<[^>]+>/g, '').replace(/!?\[([^\]]+)\]\([^)]*\)/g, '$1')
			.toLowerCase().replace(/[^\p{L}\p{M}\p{N}_\-\s]/gu, '').replace(/\s/g, '-');
		let id = base;
		let suffix = counts.get(base) ?? 0;
		while (ids.has(id)) id = `${base}-${++suffix}`;
		counts.set(base, suffix);
		ids.add(id);
	}
	return ids;
}

// Parse balanced destinations, including parentheses in external PDF URLs.
// Reference-style destinations are handled at their definitions.
export function linkDestinations(markdown) {
	const mask = proseMask(markdown);
	const links = [];
	const pattern = /!?\[[^\]\n]*\]\(/g;
	for (const match of mask.matchAll(pattern)) {
		let start = match.index + match[0].length;
		while (/\s/.test(mask[start] ?? '') && start < mask.length) start++;
		const angled = mask[start] === '<';
		if (angled) start++;
		let end = start;
		let depth = 0;
		for (; end < mask.length; end++) {
			const char = mask[end];
			if (char === '\\') { end++; continue; }
			if (angled && char === '>') break;
			if (!angled) {
				if (char === '(') depth++;
				if (char === ')' && depth-- === 0) break;
				if (/\s/.test(char) && depth <= 0) break;
			}
		}
		if (end > start) links.push({ start, end, target: markdown.slice(start, end), image: match[0][0] === '!' });
	}
	for (const match of mask.matchAll(/^ {0,3}\[[^\]\n]+\]:\s*(?:<([^>\n]+)>|(\S+))/gm)) {
		const target = match[1] ?? match[2];
		const start = match.index + match[0].indexOf(target, match[0].indexOf(':') + 1);
		links.push({ start, end: start + target.length, target, image: false });
	}
	return links.sort((a, b) => a.start - b.start);
}

export function buildWiki({ root, revision, repository = 'https://github.com/itsh-neumeier/talkops', workingCopy = false }) {
	if (!/^[a-f0-9]{40}$/.test(revision)) throw new Error('Source revision must be a full Git commit SHA.');
	root = path.resolve(root);
	const manifest = JSON.parse(readFileSync(path.join(root, 'docs/wiki/pages.json'), 'utf8'));
	const sections = new Set();
	for (const section of manifest.sections) {
		if (!section.id || !section.en || !section.de || sections.has(section.id)) throw new Error('Invalid or duplicate section.');
		sections.add(section.id);
	}
	const entries = new Set();
	if (!manifest.pages.some((entry) => entry.id === 'index')) throw new Error('Handbook index pair is required.');
	const sources = new Map();
	const slugs = new Set(['home', '_sidebar', '_footer']);
	const output = new Map();
	const assets = new Map();
	let linkCount = 0;
	const localFile = (relative) => {
		const absolute = path.resolve(root, relative);
		const inside = path.relative(root, absolute);
		if (inside.startsWith('..') || path.isAbsolute(inside)) throw new Error(`Path outside repository: ${relative}`);
		// Do not follow links into private or external directories.
		let current = root;
		for (const part of inside.split(path.sep).filter(Boolean)) {
			current = path.join(current, part);
			if (!existsSync(current)) throw new Error(`Missing local target: ${relative}`);
			if (lstatSync(current).isSymbolicLink()) throw new Error(`Symlink target is not supported: ${relative}`);
		}
		return absolute;
	};
	for (const entry of manifest.pages) {
		if (!entry.id || entries.has(entry.id) || (entry.id !== 'index' && !sections.has(entry.section))) throw new Error(`Invalid or duplicate topic: ${entry.id}`);
		entries.add(entry.id);
		for (const lang of ['en', 'de']) {
			const page = entry[lang];
			if (!page || !page.title || !new RegExp(`^docs/${lang}/[a-z0-9-]+\\.md$`).test(page.source) || !/^[A-Za-z0-9-]+$/.test(page.slug)) {
				throw new Error(`Invalid translation pair: ${entry.id}`);
			}
			if (slugs.has(page.slug.toLowerCase()) || sources.has(page.source)) throw new Error(`Duplicate page: ${page.slug}`);
			slugs.add(page.slug.toLowerCase());
			const text = readFileSync(localFile(page.source), 'utf8').replace(/\r\n/g, '\n');
			sources.set(page.source, { ...page, lang, entry, text });
		}
	}
	for (const lang of ['en', 'de']) {
		for (const file of readdirSync(path.join(root, `docs/${lang}`))) {
			if (file.endsWith('.md') && !sources.has(`docs/${lang}/${file}`)) throw new Error(`Unmapped guide: docs/${lang}/${file}`);
		}
	}
	const blobUrl = (relative) => `${repository}/blob/${revision}/${encodePath(relative)}`;
	function rewrite(page) {
		let text = page.text;
		for (const link of linkDestinations(text).reverse()) {
			const target = link.target;
			if (/^(?:https?:|mailto:|tel:)/i.test(target)) continue;
			if (/^[a-z][a-z\d+.-]*:|^\/\//i.test(target)) throw new Error(`Unsupported URL in ${page.source}: ${target}`);
			const hash = target.indexOf('#');
			const file = hash === -1 ? target : target.slice(0, hash);
			const fragment = hash === -1 ? '' : target.slice(hash);
			const relative = file ? posix(path.normalize(path.join(path.dirname(page.source), decodeURIComponent(file)))) : page.source;
			const absolute = localFile(relative);
			if (fragment && fragment !== '#' && relative.endsWith('.md')) {
				const anchor = decodeURIComponent(fragment.slice(1));
				if (!headingIds(readFileSync(absolute, 'utf8')).has(anchor)) throw new Error(`Missing anchor in ${page.source}: ${target}`);
			}
			let replacement;
			if (sources.has(relative)) replacement = (file ? sources.get(relative).slug : '') + fragment;
			else if (/\.(?:png|jpe?g|gif|svg|webp)$/i.test(relative)) {
				const name = `assets/${createHash('sha256').update(relative).digest('hex').slice(0, 16)}-${path.basename(relative)}`;
				assets.set(name, readFileSync(absolute));
				replacement = encodePath(name) + fragment;
			} else {
				replacement = lstatSync(absolute).isDirectory()
					? `${repository}/tree/${revision}/${encodePath(relative)}${fragment}` : blobUrl(relative) + fragment;
			}
			text = text.slice(0, link.start) + replacement + text.slice(link.end);
			linkCount++;
		}
		const other = page.entry[page.lang === 'en' ? 'de' : 'en'];
		const index = manifest.pages.find((entry) => entry.id === 'index')[page.lang];
		const note = workingCopy ? 'Working copy based on' : 'Source commit';
		return `${text.trimEnd()}\n\n---\n\n[${page.lang === 'en' ? 'Handbook' : 'Handbuch'}](${index.slug}) · [${page.lang === 'en' ? 'Deutsch' : 'English'}](${other.slug}) · [Source](${blobUrl(page.source)})\n\n${note}: \`${revision}\`. Generated from repository sources.\n`;
	}
	for (const page of sources.values()) output.set(`${page.slug}.md`, Buffer.from(rewrite(page)));
	const index = manifest.pages.find((entry) => entry.id === 'index');
	if (!index) throw new Error('Handbook index pair is required.');
	output.set('Home.md', Buffer.from(`# TalkOps\n\n## Deutsch\n\n[Handbuch öffnen](${index.de.slug}) — Einrichtung, Telefonie, Benutzerhandbuch und Betrieb.\n\n## English\n\n[Open the handbook](${index.en.slug}) — setup, calls, daily use and administration.\n\n${workingCopy ? 'Working copy based on' : 'Source commit'}: \`${revision}\`.\n`));
	let sidebar = '[Home](Home)\n';
	for (const lang of ['de', 'en']) {
		sidebar += `\n### ${lang === 'de' ? 'Deutsch' : 'English'}\n\n[${index[lang].title}](${index[lang].slug})\n\n`;
		for (const section of manifest.sections) sidebar += `- [${section[lang]}](${index[lang].slug}#${[...headingIds('# ' + section[lang])][0]})\n`;
	}
	output.set('_Sidebar.md', Buffer.from(sidebar));
	output.set('_Footer.md', Buffer.from(`[Repository](${repository}) · [Releases](${repository}/releases) · [Issues](${repository}/issues)\n\nEdit the sources through a repository PR. Änderungen bitte über eine Repository-PR einreichen.\n\n${workingCopy ? 'Working copy based on' : 'Source commit'}: \`${revision}\`.\n`));
	for (const [name, data] of assets) output.set(name, data);
	return { output, linkCount, pageCount: sources.size + 3 };
}

export function writeWiki(output, outputDir) {
	outputDir = path.resolve(outputDir);
	const checkParents = (destination) => {
		let current = destination;
		while (current !== path.dirname(current)) {
			if (existsSync(current) && lstatSync(current).isSymbolicLink()) throw new Error(`Output symlink: ${current}`);
			current = path.dirname(current);
		}
	};
	checkParents(outputDir);
	const inspect = (directory) => {
		for (const name of readdirSync(directory)) {
			const absolute = path.join(directory, name);
			const relative = posix(path.relative(outputDir, absolute));
			if (lstatSync(absolute).isSymbolicLink()) throw new Error(`Output symlink: ${absolute}`);
			if (lstatSync(absolute).isDirectory()) inspect(absolute);
			else if (!output.has(relative)) throw new Error(`Output directory contains unmanaged file: ${relative}. Use a new empty build directory.`);
		}
	};
	if (existsSync(outputDir)) inspect(outputDir);
	for (const [name, data] of output) {
		const destination = path.join(outputDir, name);
		checkParents(destination);
		mkdirSync(path.dirname(destination), { recursive: true });
		writeFileSync(destination, data);
	}
}

function main(args) {
	const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
	const allowed = new Set(['--check', '--release', '--output']);
	let outputDir = path.join(root, 'build/wiki');
	for (let i = 0; i < args.length; i++) {
		if (!allowed.has(args[i])) throw new Error(`Unknown option: ${args[i]}`);
		if (args[i] === '--output') {
			if (!args[i + 1] || args[i + 1].startsWith('--')) throw new Error('--output needs a directory.');
			outputDir = path.resolve(args[++i]);
		}
	}
	const git = (...params) => execFileSync('git', params, { cwd: root, encoding: 'utf8' }).trim();
	const revision = git('rev-parse', 'HEAD');
	const workingCopy = git('status', '--porcelain').length > 0;
	if (args.includes('--release') && workingCopy) throw new Error('Publication requires a clean source checkout. Commit reviewed sources first.');
	const result = buildWiki({ root, revision, workingCopy });
	if (args.includes('--check')) {
		const second = buildWiki({ root, revision, workingCopy });
		for (const [name, data] of result.output) {
			if (!data.equals(second.output.get(name))) throw new Error(`Non-reproducible page: ${name}`);
		}
	} else {
		writeWiki(result.output, outputDir);
	}
	console.log(`${args.includes('--check') ? 'Checked' : 'Built'} ${result.pageCount} Wiki pages; ${result.linkCount} local links; source ${revision}${workingCopy ? ' (working copy)' : ''}.`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	try { main(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
