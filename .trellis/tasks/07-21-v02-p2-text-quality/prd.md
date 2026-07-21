# v0.2 P2 text quality

## Goal

Improve transcription controllability and long-form structure: global glossary, quality tooling (language / presets / segment diff), and a `Chapterize` pipeline step.

## Requirements

### Glossary
- Global glossary in app config: hotwords + `from → to` replacements.
- Whisper: optional initial prompt from hotwords (when sidecar supports `-prompt`).
- Merge: optional post-replace on merged plain text.
- Job records glossary snapshot hash (no secrets).

### Quality tools
- Task-level language already exists; keep UX clear in create/settings.
- Model presets: speed / balanced / quality map to local model paths.
- Segment retry: keep previous plain text (`.prev.txt`) for simple comparison UI.

### Chapterize
- New `JobStep::Chapterize` after merge, before summarize.
- Outputs `transcript/chapters.json` + optional `chapters.md`.
- Heuristic MVP (paragraph / silence-ish SRT gaps); stable schema.
- Template variable `{{chapters}}`.

## Non-goals
- Online dictionaries, diarization, video chapter tracks, forced map-reduce.

## Acceptance
- [x] Glossary config save/load + unit tests for replace / prompt
- [x] Transcribe uses prompt; merge applies replacements when enabled
- [x] Segment prev file on re-transcribe; UI can show previous vs current
- [x] Chapterize step runnable and auto before summarize when auto_summarize
- [x] PRODUCT_SPEC P2 checkboxes; clippy / typecheck / tests pass
