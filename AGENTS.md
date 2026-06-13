## Agent skills

### Issue tracker

Issues and PRDs are tracked in GitHub Issues for `isak102/neowright`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default five-label triage vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Use the single-context layout: root `CONTEXT.md` plus root `docs/adr/`. See `docs/agents/domain.md`.

## Repository workflow

After making Rust code changes, always run `cargo fmt` and `cargo test`; both must pass before considering the work complete.
