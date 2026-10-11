# What you may change

The candidate is a TinyMemory worktree. Recall lives in
`crates/tinymemory-tools/src/recall/` (`gather.rs` collects hits per section,
`render.rs` renders and trims them into the pack) and the lifecycle in
`crates/tinymemory-tools/src/lifecycle/` (`RecallPolicy`, `standard_sections`).
The engine contract is `crates/tinymemory-api`; the CortexDB engine is
`crates/tinymemory-integrations/src/cortex/`. Changing the api or integrations
crates changes how memories are written, which forces a full re-ingest; prefer
recall-side changes.
