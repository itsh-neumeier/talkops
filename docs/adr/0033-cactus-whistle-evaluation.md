# 0033 - Cactus Whistle as a local transcription engine

- Status: Rejected
- Date: 2026-10-16
- Related to [0012](0012-aufzeichnung-und-transkription.md) and
  [0024](0024-transcription-engines-per-purpose.md). This ADR does not
  change them.

## Context

Cactus Compute released the speech-to-text model Whistle on 2026-10-02
(Apache-2.0). The model is one file of 16.9 MB (`whistle.cact`). The
`needle` engine runs it on a CPU without a GPU. The model supports English,
German, French, Spanish, Italian, Dutch and Polish. It gives word times with
probabilities, keyword biasing and a stream mode.

The user asked if TalkOps can use Whistle as an additional local engine for
all transcription purposes (`live`, `recording`, `voicemail` and the second
passes). The vendor compares Whistle only with Whisper `base`. The vendor
publishes no independent results and no results for telephone audio.

Before a decision, we did a test (phase 0) on the hardware of the
development container.

### Test setup

- CPU: 4 cores, Intel Xeon at 2.80 GHz (AVX-512), 15 GB RAM.
- Whistle: `needle` CLI from `huggingface.co/Cactus-Compute/needle3`
  (`linux-x86_64/needle`, SHA-256
  `3531889c4d677acc47905b21c79741b52fd9af2c54ba177f54f83c0bc12e84c7`) and
  `whistle.cact` from `huggingface.co/Cactus-Compute/whistle` (SHA-256
  `b6e02f048568ac5d01a2042556c658061e699acbc0aa2a1439f52f3d461dffeb`),
  both downloaded on 2026-10-16. The `linux-arm64/needle` file has the
  SHA-256 `ab21ca5272e7699ed4972246d95e75dd09a7b81f5ef8b8a6262e6ff57c600593`.
- Whisper: whisper.cpp v1.9.4 (the version of the media worker image) with
  `ggml-base.bin` and `ggml-large-v3-turbo-q5_0.bin` (TalkOps `fast`),
  4 threads, no VAD.
- German audio: the first 23 clips of a spread selection of the MLS German
  test set (334 s of audio, 732 words).
- English audio: 29 clips of the LibriSpeech `test-clean` set (252 s of
  audio, 726 words).
- Each clip was used in two forms: broadband (16 kHz) and telephone
  (8 kHz G.711 mu-law, converted back to 16 kHz PCM). The language was set
  for each engine. The test removed punctuation and capital letters before it
  calculated the word error rate (WER).

### Results

| Engine | German WER broadband | German WER telephone | German time | English WER broadband | English WER telephone |
| --- | --- | --- | --- | --- | --- |
| Whistle | 20.1 % | 23.2 % | 0.27 x real time | 4.5 % | 6.1 % |
| Whisper `base` | 15.6 % | 17.1 % | 0.23 x real time | 4.5 % | 5.5 % |
| Whisper `large-v3-turbo-q5_0` | 5.3 % | 5.1 % | 2.4 x real time | Not measured | Not measured |

Other results:

- For one German file of 127 s, Whistle used 44.8 s. Whisper `base` used
  17.7 s. Whistle needs much more time than `base` for long audio.
- Whistle used approximately 82 MB of RAM.
- Whistle returned empty text for 20 s of silence and for 10 s of noise.
- Keyword biasing corrected names (for example "Münchhausen" and "Zauber").
- The `needle` binary links only `libc`, `libm`, `libpthread` and `libdl`.
  It has no HTTP client and no TLS library. It ran correctly without a
  network (`unshare -n`). The Python package `cactus-needle` sends usage
  counts, but the binary does not use that package.
- The `needle` CLI does not read mu-law WAV files. The media worker always
  writes 16-bit PCM, so this is not a problem.
- The C++ source code of the engine is not public. Only built binaries for
  each platform are available.

## Options

1. Add Whistle as an additional local model for all purposes, marked as
   experimental.
2. Add Whistle only for the live preview.
3. **Do not add Whistle now. Do the test again with a new Whistle release.**

## Decision

Option 3.

German is the main language of TalkOps installations. On German telephone
audio, Whistle has approximately one third more word errors than Whisper
`base`. Whisper `base` is the current default for the live preview. On x86
CPUs, Whistle is not faster than `base`, and on long audio it is much slower.
Thus Whistle gives no better option for any purpose. Only the small RAM use
is better, and TalkOps installations seldom have so little RAM that this is
important. A new engine also adds a built binary without public source code
and a second output format to maintain.

## Consequences

- TalkOps has no code, image or migration changes for Whistle.
- The local engines stay whisper.cpp with the models of ADR 0024.
- To do the test again, use the same setup with the new files. Add Whistle
  only if it is better than Whisper `base` on German telephone audio.
  If the test is positive, write a new ADR that supersedes this ADR.
