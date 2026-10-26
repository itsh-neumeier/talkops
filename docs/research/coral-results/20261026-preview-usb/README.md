# Coral USB preview latency and quality

The actual USB Accelerator completed the predefined quality and latency
comparison. A stable text preview can use about 3–5 seconds of delay.
This delay alone does not preserve independent-clip recognition quality in
a continuous stream. Do not enable this prototype in production yet.

These measurements use the earlier Python benchmark prototype. They are
reference evidence for the optional Rust complete-word preview. Rust unit
and WebSocket tests check the port separately. They do not establish the
same measured latency or WER for the Rust service on real hardware.

## Method

Use the unchanged overnight German candidate. Its input has 512 frames and
256 frames of overlap. Keep the centre output frames and publish complete
CTC words. Hold an open word. Check that later output preserves each
published prefix. Initial artificial silence moves the first window earlier.
It does not reduce the input shape or change model weights.

The quality test uses all 200 held-out Common Voice German clips after the
existing telephone A-law preparation. There are 1,784 reference words.
Common Voice supplies the references. No ElevenLabs text or private audio
is used. The unpadded final decode matched the existing recognizer on the
first five real clips and on the unit fixtures.

The real-time test uses two channels on one Coral, with a four-CPU quota.
Each channel receives 200 ms chunks and runs for at least two minutes.
The same 41 clip occurrences provide 265 reference words in every run.
These are 27 unique source clips; the channel selections overlap.
Clips have 1.5 seconds of silence between them. They are a small corpus
replay, not real conversations or a live SIP call. The initial comparison
keeps recognition running across every pause. A second comparison uses
the existing causal TalkOps turn detector: 300 ms lead-in, 700 ms silence,
15 s maximum turn. The detector does not receive reference text or clip
boundaries. No shorter fixed turn limit is introduced.

All device runs are sequential. The USB link is 5,000 Mbit/s. Earlier Coral
tests used 480 Mbit/s, so their timing is not a controlled comparison.
See [environment.json](environment.json) for the image and source selection.
JSON records omit transcripts, audio and per-word event arrays.

## Independent-clip quality

| Initial silence | WER | Word edits | Difference from unpadded |
| --- | ---: | ---: | ---: |
| None | 22.93% | 409 | Reference |
| 1.28 s | 24.44% | 436 | +1.51 percentage points |
| 2.56 s | 24.05% | 429 | +1.12 percentage points |

These scores come from [quality-200.json](quality-200.json). They are final
texts from separate clips, without turn detection. They do not establish
live WER. The earlier native USB result was 22.98%, or one additional edit.
Do not interpret that small difference as a model improvement.

## Continuous recognition

| Initial silence | Live subset WER | First speech to text p50 / p95 | Estimated word-end delay p50 / p95 |
| --- | ---: | ---: | ---: |
| None | 45.66% | 3.74 / 5.40 s | 3.00 / 4.16 s |
| 1.28 s | 46.79% | 3.64 / 5.00 s | 3.02 / 4.16 s |
| 2.56 s | 45.66% | 3.61 / 5.42 s | 2.96 / 4.17 s |

All 41 reference intervals had an associated output word in these runs.
Average CPU use was 0.177–0.180 cores. Maximum final backlog was 0.370 s.
No published prefix changed. The 1.28 s policy produced the first call text
after 4.19–4.36 s, versus 5.39–5.54 s without padding. The 2.56 s policy
was inconsistent: 2.95 s on one channel and 5.54 s on the other.

Read [live-0.json](live-0.json), [live-128.json](live-128.json) and
[live-256.json](live-256.json) for the complete aggregate results.

## Recognition with pause resets

| Initial silence per turn | Live subset WER | First speech to text p50 / p95 | Estimated word-end delay p50 / p95 |
| --- | ---: | ---: | ---: |
| None | 28.68% | 3.93 / 5.07 s | 2.30 / 4.06 s |
| 1.28 s | 30.94% | 3.74 / 4.23 s | 2.13 / 3.23 s |

Both runs use the same audio as the continuous runs. Resetting at causal
pauses reduces WER substantially in this sample. Earlier startup trades
about 2.26 percentage points of WER for a lower delay. Do not generalize
this small result to all calls.

Both runs commit 37 turns. Four of 41 reference intervals have no associated
output word. Their first-text delays are absent from the timing percentiles.
The complete channel-level WER still includes their reference words.
There is no correction of published prefixes. CPU use is 0.148 cores for
the unpadded run and 0.189 cores for the faster-start run. Backlog is below
1 ms. These results come from [pause-0.json](pause-0.json) and
[pause-128.json](pause-128.json).

The original final recognizer processed exactly the same 41 clip occurrences
as independent files. Its WER is 22.26%; see
[matched-file-control.json](matched-file-control.json). Thus, the difference
from live WER is not explained by selection of harder source clips alone.
Pause resets retain more quality than continuous processing, but they still
lose quality relative to this matched control. The experiment changes window
alignment, silence context and turn detection together. It does not identify
one root cause for the remaining errors or prove that all four missing
intervals were discarded by VAD.

## Timing limits

First speech uses the first 20 ms frame above a fixed RMS threshold.
Words use CTC time estimates from the recognizer. These are approximate
timing measures, without human-aligned word references. Word delay includes
only emitted words. First-text timing includes only intervals with a word
whose estimated centre lies inside the reference interval. Missing intervals
must be read with the timing values; they remain errors in channel-level WER.
The word delay does not measure whether each emitted word is correct.

CPU excludes initial WAV loading and resampling. Network, SIP transport,
Docker model initialization and UI rendering are outside the live timing.
The two channel tests do not establish capacity for more calls or a p95 SLA.

## Decision

Use about 4–5 seconds as an initial prototype budget for this model. The
unpadded pause-reset run has an estimated word-delay p95 of 4.06 seconds.
Its first-text maximum is 6.32 seconds. Do not promise a strict 3-second
limit or a strict 5-second maximum. Initial padding improves the pause-reset
delay at an accuracy cost. In continuous mode, more initial padding did not
reduce steady-state word delay. Increasing the wait alone does not fix WER.

Keep the unpadded profile as the quality reference. Treat 1.28 s initial
padding as a faster-start experiment with an explicit accuracy cost.
The current measurements do not establish good enough recognition for
telephone deployment. Validate segmentation and recognition together on
more labeled telephone audio before integration.

In the earlier benchmark task, all 50 tool tests passed in the existing Linux image. The Windows
run passed with three POSIX platform skips. The test for pause resets checks
complete word output and absolute timestamps across a pause and final flush.

Language review used the repository writing rules. The full ASD-STE100
standard was unavailable; its full language review remains pending.
