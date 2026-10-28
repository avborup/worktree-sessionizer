# wt

Git worktree + tmux session manager. Create worktrees, spin up ready-to-go tmux sessions, and switch between them with a fuzzy picker.

## Install

```sh
cargo install --path .
```

## Quick start

Register a project:

```sh
wt project add --name myapp --repo-path ~/coding/myapp --pane-command "./nix/develop --command fish"
```

Create a worktree and session:

```sh
cd ~/coding/myapp
wt new feat/my-feature           # check out existing branch
wt new main feat/my-feature      # create new branch from main
```

Switch between worktrees:

```sh
wt switch
```

## Commands

| Command | Description |
|---------|-------------|
| `wt new <branch>` | Create worktree + tmux session |
| `wt new <base> <branch>` | Create worktree on new branch from base |
| `wt attach [path]` | Track an existing worktree (defaults to cwd) |
| `wt switch` | Fuzzy picker to switch between worktrees |
| `wt list` | List tracked worktrees |
| `wt list -v` | Verbose list with full details |
| `wt info` | Show info about the current project |
| `wt info --project-name` | Print only the project name (for shell interpolation) |
| `wt remove <name>` | Remove a worktree by branch or session name |
| `wt remove <name> --branch` | Remove worktree and delete the branch |
| `wt remove <name> --force` | Discard worktree changes and untracked files; with `--branch`, force-delete the branch |
| `wt prune` | Remove worktrees unused for 30+ days |
| `wt prune --days 14` | Custom staleness threshold |
| `wt prune --branch` | Remove stale worktrees and delete their branches |
| `wt prune --force` | Discard worktree changes and untracked files; with `--branch`, force-delete branches |
| `wt gc <name>` | Run the project's named gc command in the repo and every worktree |
| `wt gc <name> --dry-run` | List the directories `wt gc` would run in |
| `wt project add` | Register a project |
| `wt project list` | List registered projects |
| `wt project remove` | Unregister a project |

## tmux keybinding

Bind `wt switch` to a tmux popup for quick access:

```tmux
bind-key -n C-o display-popup -E -w 80% -h 60% "wt switch"
```

Add this to `~/.tmux.conf`, then `tmux source ~/.tmux.conf`. Press `Ctrl-o` from any pane to open the fuzzy picker.

## Session layout

Each new worktree gets a tmux session with:

- **Window 0**: two panes (left: shell, right: shell)
- **Window 1** ("server"): single shell pane

All panes run the project's configured `pane_command` (e.g. `./nix/develop --command fish`).

## Project config

Config lives at `~/Library/Application Support/wt/config.toml`:

```toml
worktrees_root = "/Users/you/coding/.worktrees"
prune_after_days = 30

[[projects]]
name = "myapp"
repo_path = "/Users/you/coding/myapp"
pane_command = "./nix/develop --command fish"
cow_clone = ["node_modules", ".venv", ".env", ".env.local"]
symlink = [".eslintcache"]
```

`cow_clone` entries are copied using macOS `clonefile()` (instant, copy-on-write). `symlink` entries are symlinked from the main repo.

`gc` is an optional table of named shell commands. `wt gc <name>` runs one with `sh -c` in the main repo and every git worktree of the project, then reports the disk space freed:

```toml
[projects.gc]
light = "find target -path '*/deps/*.rcgu.o' -mtime +3 -delete"
full = "rm -rf target"
```

## Metadata

Worktree state is tracked in `~/Library/Application Support/wt/state.toml`. This file is managed by the tool — don't edit it by hand unless you need to backdate `last_used_at` for testing `prune`.
