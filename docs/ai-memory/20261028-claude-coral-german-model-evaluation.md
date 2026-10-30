# German telephone STT model evaluation for Coral

## Task state

- Date: 2026-10-28
- Tool: Claude Code
- Status: Research complete; no code changed; no experiment run
- Branch: codex/citrinet-german-training (working tree only, not committed)
- Related summary: `20261028-codex-citrinet-german-training.md`

## Goal

Find a German speech recognition model that runs on a Coral Edge TPU,
can be retrained on Common Voice German and telephone audio, and can be
redistributed with TalkOps. Diagnose the failed Citrinet transfer run.

## License results (checked 2026-10-28)

- `stt_de_quartznet15x5` (NGC): only "NGC Terms of Use unless another
  license is clearly specified". No CC license or redistribution grant found
  (NGC API model description). Not cleared for public redistribution.
- `stt_de_citrinet_1024` (NGC): same NGC Terms of Use text.
- `nvidia/stt_en_citrinet_256_ls`, `stt_en_citrinet_512_ls` (36M),
  `stt_en_conformer_ctc_small` (13M): CC-BY-4.0 (Hugging Face cards).
- `nvidia/stt_de_fastconformer_hybrid_large_pc` (115M, CV12 test WER 5.10 %),
  `canary-180m-flash` (182M), `parakeet-tdt-0.6b-v3` (600M): CC-BY-4.0.
- Scribosermo German QuartzNet/Conformer: CC BY-SA 4.0 by default, may be
  double licensed by NVIDIA, some datasets non-commercial. Excluded.

## Coral evidence in this repository

- German QuartzNet 15x5, 512-frame window: 189 of 190 ops on the TPU, one
  subgraph, only the final LOG_SOFTMAX on the CPU. On-chip 7.4 MB, off-chip
  18.3 MB (`build/coral-work/compile-de-512.log`).
- USB Coral file run: RTF 0.187, WER 30.6 % / CER 11.5 % on 8 utterances
  (`build/coral-work/usb-de-512.log`). Live p95 latency 3.2-5.4 s across
  profiles (`build/coral-work/coral-latency-20261026/`).
- Thus off-chip parameter streaming of about 18 MB is shown to be usable.
- No Citrinet model has been compiled for the Edge TPU yet.

## Citrinet diagnosis (from code and logs, not yet tested)

1. Budget: 1,800 updates x 8 clips = 14,400 clips, under one epoch of the
   19,999-clip set. Early stopping on `val_wer` (patience 12) stops while
   WER stays near 1.0 during the CTC blank phase. Monitor `val_loss`/CER.
2. Logged loss is likely divided by gradient accumulation: losses doubled
   (about 20 to about 45) exactly when accumulation changed from 8 to 4 at
   the resume. Log the unscaled loss per target character.
3. Constant AdamW 1e-4 after a 100-update decoder warmup is low for a new
   CTC head and modified upper blocks. No schedule.
4. Stride change in the last stage (8 blocks) halves their time receptive
   field. Alternative: keep 8x and use a German SentencePiece vocabulary.
5. Frozen BN statistics come from 16 kHz LibriSpeech. G.711 input shifts
   activations. Option: re-estimate BN statistics before training.
6. Checked by code reading: labels, blank index (NeMo appends blank last),
   strict encoder/preprocessor loading, dither 0, length check at 4x.
7. Citrinet SE uses global context; a fixed TPU window changes this context.

## Proposed experiment

- S0: compare encoder outputs of the original `.nemo` and the rebuilt model
  with the stride unchanged on 5 clips. Expected max abs difference < 1e-4.
- Overfit 16 clean clips, batch 1, no accumulation, no early stopping,
  AdamW 1e-3 constant, 1,500 updates. GPU 0: current 4x character model.
  GPU 1: original 8x with German unigram SentencePiece (128 tokens).
- Pass: train-set CER <= 5 % and WER <= 15 % by update 1,500.

## Open work

- Implement and run S0 and the overfit test.
- Build one evaluation script and re-measure QuartzNet and every candidate
  on the same 200 test clips, same text normalization, same window mode.
- STE review: full standard unavailable; not reviewed.
