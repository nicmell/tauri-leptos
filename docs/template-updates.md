# Pulling template updates

An app generated from this template keeps its own history. The template stays reachable as a second remote:

```bash
git remote add template <template repository URL>
git fetch template
git merge template/main          # or cherry-pick single commits
```

The URL is a placeholder on purpose. The rename rewrites every copy of the template's name, and that includes its URL.

Conflicts land where the rename touched the tree: the crate names, the bundle identifier, and the deb and systemd paths. Two habits keep them cheap:

- Keep app code out of the demo. The demo is `crates/ui/src/demo.rs` with its websocket section and session, the demo messages in `crates/protocol`, the `greet` command with its binding, and the logos. Deleting them is the documented start, so a template commit that touches them conflicts with nothing of yours.
- When a merge gets noisy, cherry-pick. Each template commit holds one feature (see CLAUDE.md), so `git cherry-pick <commit>` is often cleaner than a merge of a range.

The rename is deterministic. If you run `scripts/rename-app.sh` with the same flags on a file from the template, you get the text that your app already has. Keep the flags in your README, or in the commit that renamed the app, so that the next merge can use them again.
