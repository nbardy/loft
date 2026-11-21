# Integration close · 20 November 2026

| Repository | Published revision and verification | Still open |
|---|---|---|
| cleanup_tools | Local `runtime-guard` at `f9132e5` includes guard `7a9ceb4` and the runbook line. All 117 unittests passed under agent-build with the policy archive Python. The first run lacked huggingface_hub. | No push. No remote is configured, and local main has edits in nine affected paths. Reconcile those edits and supply the destination. |
| freeze_sampler | Pushed master `7821823` (fs1 + fs2). Release build and all 10 Rust tests passed under agent-build. Helper plist and installer syntax passed. Clean fs2 worktree removed. | Owner install, live privileged stackshots, FDA lineage and tailspin capture. |
| Cambium | Pushed release/v0.10 `ee292e8` over `f6bfb61`, including `fb755e9`, `d5b56b9` and `d2a41cf`. `agent-build go test ./...` exited 0. Clean archive built and reported 0.10.1. | Owner binary replacement. The dirty release checkout remains at `f6bfb61`. |
| Loft | Pushed main `04b8bdf` (goals and handoff). IX4 audit `2f809a4` scored `3c24a11`. std1b and std1e merged. Wrap-up static checks exited 0, including 302/302 expected results in each grammar recognizer. Pictures 19/19, Twin 36/36, Speed 6/12, Load 23/33, Truth 34/34. | Six contended timings, Load exit 4, two baseline native failures (159 passed). std1c/d, port1 and v15a/b/c held. stdaot1 deferred. [Handoff](HANDOFF.md) |

## Owner · freeze-helper and recorder

Run in one shell. The export keeps the other session’s checkout intact.

```sh
fs_src=$(mktemp -d) && git -C ~/git/freeze_sampler archive -o "$fs_src/src.tar" 782182376c2c51ed4f55658a6bb2c690a9d6b13c
(cd "$fs_src" && tar -xf src.tar && ~/.local/bin/agent-build cargo build --release && target/release/freeze_sampler check-stall && sudo target/release/freeze-helper --probe-stackshot && sudo bash install-helper.sh)
```

After the helper installs, open System Settings → Privacy & Security → Full Disk Access. Add and enable both `/usr/bin/eslogger` and `/usr/local/libexec/freeze-helper`. Run `/usr/bin/tailspin info`. To arm tracing if disabled, run `sudo /usr/bin/tailspin enable`. After the first lineage event, verify the ring reports `running`.

```sh
(cd "$fs_src" && target/release/freeze_sampler check-stall && sudo launchctl kickstart -k system/com.nbardy.freeze-helper && sudo bash install-daemon.sh --with-helper && bash install-ui.sh)
sudo sh -c 'cat /var/db/freeze-helper/rings-*/lineage.json'
```

## Owner · Cambium binary

```sh
(
  set -eu
  src=$(mktemp -d)
  git -C ~/git/_wt/cambium-nolsof archive -o "$src/src.tar" ee292e8f873ac504f7d1276e5650d3039d2f57e3
  cd "$src"; tar -xf src.tar
  ~/.local/bin/agent-build go build -trimpath -o cambium ./cmd/cambium
  ./cambium version
  dst="$HOME/.local/bin/cambium"
  backup=$(mktemp "$dst.backup.XXXXXXXX"); cp -p "$dst" "$backup"; cmp "$dst" "$backup"; echo "Backup $backup"
  tmp=$(mktemp "$dst.new.XXXXXXXX") && install -m 755 ./cambium "$tmp" && uv run --no-project python -c 'import os,sys; os.replace(*sys.argv[1:])' "$tmp" "$dst" && cmp ./cambium "$dst"
)
```
