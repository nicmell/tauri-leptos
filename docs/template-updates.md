# Pulling template updates

An app generated from this template keeps its own history; the template
stays reachable as a second remote:

```bash
git remote add template https://github.com/nicmell/tauri-leptos.git
git fetch template
git merge template/main          # or cherry-pick single commits
```

Conflicts land exactly where the rename touched the tree (crate names,
the bundle identifier, the deb/systemd paths). Two habits keep them
cheap:

- **Keep app code out of the demo files.** `crates/app-core/src/server/demo.rs`
  and `crates/ui/src/demo.rs` are the template's, and deleting them is
  the documented start — a template commit that touches them then
  conflicts with nothing of yours.
- **Cherry-pick when a merge gets noisy.** Template commits are one
  feature each (`One feature per commit` in CLAUDE.md), so
  `git cherry-pick <commit>` is usually cleaner than merging a range.

The rename is deterministic: re-running `scripts/rename-app.sh` with the
same flags on a file taken from the template produces exactly the text
your app already has. Keep the flags you used in your README (or in the
commit that renamed the app) so the next merge can reuse them.
