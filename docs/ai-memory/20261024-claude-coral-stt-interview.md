# Coral Edge TPU speech recognition: interview and development prompts

## Task state

- Task ID: 20261024-claude-coral-stt-interview
- Date: 2026-10-24
- Tool: Claude Code
- Status: Completed
- Branch: claude/nifty-babbage-ymchlw
- Base commit: 8f32ec7 (release 1.8.10)
- Related commits or PR: Pending

## Goal

Find out if Google Coral can do German and English speech recognition for
TalkOps, mainly for the live transcription. Interview the user about the
implementation. Write development prompts in the standard format.

## Result

- Feasibility: possible only as a hybrid. Whisper does not run on the Edge
  TPU. Small int8 CTC models can run with lower accuracy. Local Whisper does
  the second pass.
- Interview completed in four rounds. The user added a second goal: the new
  server must also supply a "virtual TPU over IP" for other software such as
  Frigate.
- Two prompts in `docs/proposals/coral-transcription-dev-prompts.md`:
  Prompt A for the new project `itsh-neumeier/coralapi-server` (Rust, MIT),
  Prompt B for the TalkOps integration (connection kind `coral`).
- Follow-up: the user selected Prompt C as the first step. Prompt C puts a
  `coral-stt` service into TalkOps on the same server (own crate and image,
  loopback only, Compose profile `coral`). It uses the same API contract.

## Changes

- `docs/proposals/coral-transcription-dev-prompts.md`: Feasibility, interview
  decisions, shared API contract, Prompt A and Prompt B.
- `docs/ai-memory/20261024-claude-coral-stt-interview.md`: This summary.

## Decisions

- Hybrid: Coral for live and first pass, Whisper for the second pass.
- Separate project, used as an optional sidecar (Compose profile `coral`) or
  an external LAN server.
- Automatic DE/EN detection; Realtime subset of ADR 0024; new kind `coral`;
  fall back to local Whisper.
- Phase 0 benchmark is a hard gate before more work.
- Virtual TPU: Frigate ZMQ detector, generic remote TPU API, later a
  libedgetpu shim, USB/IP documentation. TPU pools with roles `stt`,
  `vision`, `shared`. API key per client and optional TLS.
- Rust runtime by user choice. Python is permitted for offline model
  conversion tools. `unsafe` only in the FFI crate.
- Prompt C: TalkOps forbids `unsafe`. The prompt asks for a safe wrapper
  crate, or an ADR and user approval before a narrow FFI exception.

## Checks

- Not run: code checks. The task changed only documentation.
- STE review: manual review; full standard unavailable.

## Dependencies and open work

- Prompt B depends on the API contract. It can start against a test double.
- The new repository is outside the scope of this session.

## Next action

Start Prompt C, phase 0, in a new TalkOps chat. Prompts A and B stay for later.

## References

- ADR 0019, ADR 0022, ADR 0024
- `docs/ai-memory/20261013-claude-transcription-engines.md`
