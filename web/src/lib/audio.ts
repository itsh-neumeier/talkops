// Audio clips in the browser: decode any format the browser plays, convert
// to 16 kHz mono 16-bit WAV (what the server accepts and the phone system
// plays), record from the microphone, and compute waveform bars.

import { ApiError, upload } from '#lib/api.ts';

export interface Clip {
	id: string;
	source: 'tts' | 'upload' | 'recording';
	text: string;
	language: 'de' | 'en';
	voice: 1 | 2;
	status: 'pending' | 'ready' | 'failed';
	duration_ms: number;
	created_at: string;
}

export interface Voice {
	language: 'de' | 'en';
	voice: 1 | 2;
	name: string;
	gender: 'female' | 'male';
}

export const clipUrl = (id: string) => `/api/v1/audio/clips/${id}/audio`;

/** Built-in music on hold. */
export interface MusicTrack {
	id: string;
	title: string;
	/** False until FreeSWITCH has started once. */
	available: boolean;
}

export const musicUrl = (id: string) => `/api/v1/audio/music/${id}`;

const RATE = 16000;

/** Decodes audio data and resamples it to mono (16 kHz by default), at most
 * `maxSeconds` long. */
export async function decode(
	data: ArrayBuffer,
	rate = RATE,
	maxSeconds = Infinity
): Promise<AudioBuffer> {
	const ctx = new AudioContext();
	try {
		const decoded = await ctx.decodeAudioData(data);
		const length = Math.max(1, Math.ceil(Math.min(decoded.duration, maxSeconds) * rate));
		const offline = new OfflineAudioContext(1, length, rate);
		const src = offline.createBufferSource();
		src.buffer = decoded;
		src.connect(offline.destination);
		src.start();
		return await offline.startRendering();
	} finally {
		void ctx.close();
	}
}

/** Encodes the first channel as 16-bit PCM WAV. */
export function encodeWav(buffer: AudioBuffer): Blob {
	const samples = buffer.getChannelData(0);
	const out = new DataView(new ArrayBuffer(44 + samples.length * 2));
	const text = (offset: number, s: string) =>
		[...s].forEach((c, i) => out.setUint8(offset + i, c.charCodeAt(0)));
	text(0, 'RIFF');
	out.setUint32(4, 36 + samples.length * 2, true);
	text(8, 'WAVE');
	text(12, 'fmt ');
	out.setUint32(16, 16, true);
	out.setUint16(20, 1, true); // PCM
	out.setUint16(22, 1, true); // mono
	out.setUint32(24, buffer.sampleRate, true);
	out.setUint32(28, buffer.sampleRate * 2, true);
	out.setUint16(32, 2, true);
	out.setUint16(34, 16, true);
	text(36, 'data');
	out.setUint32(40, samples.length * 2, true);
	samples.forEach((s, i) => {
		const v = Math.max(-1, Math.min(1, s));
		out.setInt16(44 + i * 2, v < 0 ? v * 0x8000 : v * 0x7fff, true);
	});
	return new Blob([out.buffer], { type: 'audio/wav' });
}

/** Converts any playable audio (file or recording) to a WAV upload. */
export async function toWav(blob: Blob): Promise<Blob> {
	return encodeWav(await decode(await blob.arrayBuffer()));
}

/** Uploads a WAV as a new clip. */
export function uploadClip(wav: Blob, source: 'upload' | 'recording'): Promise<Clip> {
	const form = new FormData();
	form.append('source', source);
	form.append('file', wav, 'clip.wav');
	return upload<Clip>('/audio/clips', form);
}

/** Peak level (0…1) of `count` equal slices: the bars of a waveform. */
export function peaks(buffer: AudioBuffer, count: number): number[] {
	const data = buffer.getChannelData(0);
	const size = Math.max(1, Math.floor(data.length / count));
	const bars: number[] = [];
	for (let i = 0; i < count; i++) {
		let max = 0;
		for (let j = i * size; j < Math.min(data.length, (i + 1) * size); j++) {
			max = Math.max(max, Math.abs(data[j]));
		}
		bars.push(max);
	}
	const top = Math.max(...bars, 0.01);
	return bars.map((b) => b / top);
}

/** Fetches an audio resource (with the session cookie). */
export async function fetchAudio(url: string): Promise<Blob> {
	const res = await fetch(url, { credentials: 'same-origin' });
	if (!res.ok) throw new ApiError(res.status, 'error', res.statusText);
	return res.blob();
}

/** A microphone recording; `stop()` resolves with the WAV. */
export interface Recording {
	stop(): Promise<Blob>;
	cancel(): void;
}

export async function record(): Promise<Recording> {
	const stream = await navigator.mediaDevices.getUserMedia({
		audio: { echoCancellation: true, noiseSuppression: true }
	});
	const recorder = new MediaRecorder(stream);
	const chunks: Blob[] = [];
	recorder.ondataavailable = (e) => chunks.push(e.data);
	const done = new Promise<Blob>((resolve) => {
		recorder.onstop = () => resolve(new Blob(chunks, { type: recorder.mimeType }));
	});
	recorder.start();
	const release = () => stream.getTracks().forEach((t) => t.stop());
	return {
		async stop() {
			recorder.stop();
			const blob = await done;
			release();
			return toWav(blob);
		},
		cancel() {
			if (recorder.state !== 'inactive') recorder.stop();
			release();
		}
	};
}

export function formatDuration(ms: number): string {
	const s = Math.max(0, Math.round(ms / 1000));
	return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}
