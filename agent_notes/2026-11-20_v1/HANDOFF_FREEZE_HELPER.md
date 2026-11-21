# Freeze Helper & Recorder Handoff (2026-11-21)

Standalone handoff for continuing/installing `freeze_sampler` (`freeze-helper` + daemon + UI) without colliding with active sessions.

## 1. Current Repository & Worktree State (`~/git/freeze_sampler`)

| Path | Branch | HEAD | State | Notes |
|---|---|---|---|---|
| `~/git/freeze_sampler` | `review-gather-first` | `cb36023` | behind `master` by 7 commits | Primary checkout (keep intact or fast-forward after checking any local state) |
| `origin/master` & `master` | `master` | `7821823` | **pushed & tested** | Includes `fs1` (`stall-marker` `f0ef948`) + `fs2` (`incident-capture` `7821823`): process-polling gate during stalls, stall evidence in crash reviews, prearmed rings, post-reboot recovery |
| `~/git/_wt/freeze-lineage-optin` | `lineage-optin` (base `7821823`) | `7821823` + **3 dirty files** | **ACTIVE OTHER WORK** | Uncommitted edits in `README.md`, `install-helper.sh`, `src/bin/freeze_helper/main.rs` (`+25/-11`). **Do NOT overwrite, reset, or remove.** Coordinate/merge before final owner install so the lineage opt-in behavior is included |
| `~/git/_wt/freeze-stallmarker-final` | `stall-marker` | `f0ef948` | clean, ancestor of `master` | Safe to remove via `cambium remove --safe` once convenient (all commits already in `7821823`) |

## 2. What Is Verified at `7821823` (M4, macOS 15.5 / `24F74`)

- Release build, all 10 Rust tests, recorder E2E, helper E2E, and launchd plist syntax passed.
- Synthetic `STALL` published in **0.245 s**; second retained shot at **60.146 s**; decode in **9.816 s**; bundle **4.08 MB**.
- Simulated reboot recovered an interrupted snapshot from the previous boot's rings and decoded the retained shot.
- 5-minute idle helper cost (feeds inactive): **0.00050% machine CPU**, **3.47 MiB peak RSS**.
- Filtered log child (opt-in `--log`): **0.281% CPU** (excluding `logd`), **6.83 MiB peak RSS**, ~144 MB/day ring.

## 3. What Remains Unverified / Pending Owner Privileges

1. **Reconcile `lineage-optin` (`~/git/_wt/freeze-lineage-optin`):** Finish, test, commit, and merge the 3 in-progress edits (`README.md`, `install-helper.sh`, `src/bin/freeze_helper/main.rs`) into `master` before building the installed helper (or install from `lineage-optin`'s final commit).
2. **Privileged direct stackshot probe:** Returned `EPERM` unprivileged; needs `sudo` probe on live host.
3. **Full Disk Access (FDA) + `eslogger` lineage:** Needs owner GUI grant in System Settings for `/usr/bin/eslogger` and `/usr/local/libexec/freeze-helper`.
4. **`tailspin` tracing:** Was disabled on this Mac; needs `sudo /usr/bin/tailspin enable` and live save/decode check.
5. **Daemon + UI install:** Install `com.nbardy.freeze-helper` LaunchDaemon, re-install daemon with `--with-helper`, and install UI.

## 4. Exact Install & Verification Commands (Run After Merging `lineage-optin` or Using Its Commit)

Replace `REF` below with the merged `master` SHA (currently `7821823` before `lineage-optin` lands):

```sh
# 1. Build cleanly from a commit export (never mutates active worktrees)
fs_src=$(mktemp -d)
git -C ~/git/freeze_sampler archive -o "$fs_src/src.tar" REF
(
  cd "$fs_src" && tar -xf src.tar &&
  ~/.local/bin/agent-build cargo build --release &&
  target/release/freeze_sampler check-stall &&
  sudo target/release/freeze-helper --probe-stackshot &&
  sudo bash install-helper.sh   # add --log and/or --lineage per lineage-optin flags
)
```

Then grant Full Disk Access (GUI):
- **System Settings → Privacy & Security → Full Disk Access → `+` → `Cmd+Shift+G`**
- Add and enable:
  - `/usr/bin/eslogger`
  - `/usr/local/libexec/freeze-helper`
- Check & arm tailspin:
  - `/usr/bin/tailspin info`
  - If disabled: `sudo /usr/bin/tailspin enable`

Then restart helper, verify live lineage ring, and install daemon + UI:

```sh
(
  cd "$fs_src" &&
  target/release/freeze_sampler check-stall &&
  sudo launchctl kickstart -k system/com.nbardy.freeze-helper &&
  sudo bash install-daemon.sh --with-helper &&
  bash install-ui.sh
)
sudo sh -c 'cat /var/db/freeze-helper/rings-*/lineage.json'   # expect "running"
```

Uninstall / rollback:
```sh
sudo bash uninstall-helper.sh && sudo bash install-daemon.sh
```

## 5. Safety Rules for Any Agent Touching This Repo

- **Zero `ps`/`pgrep`/`lsof`/raw `spindump` polling during a stall:** all 5 macOS 15.5 freezes on Nov 20 were XNU `proc_find + 436` kernel lock convoys fed by concurrent process-table scans. If `~/Library/Logs/FreezeSampler/STALL` exists, stop all process inspection immediately.
- Queue all Cargo builds through `~/.local/bin/agent-build`.
- Remove finished worktrees only via `git cambium remove --safe <name>` from `~/git/freeze_sampler` after verifying commits are merged.
