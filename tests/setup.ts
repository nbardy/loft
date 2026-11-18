// Loaded before every test file (bunfig.toml). These tests create GPU pipelines, which took over 10 s under a load
// average above 60 on a shared machine, against bun's default of 5 s per test. The `timeout` key in bunfig.toml was
// not being honoured (a relief test still timed out at 5 s on 2026-11-11), so the limit is set in code here.
import { setDefaultTimeout } from "bun:test";
import { constants, openSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

// Dawn shader compilation shares the Mac's unified memory across worktrees. Acquire the kernel lock before any
// test module loads. Keep the descriptor open until process exit (including SIGKILL); never unlink the lock file.
if (process.platform === "darwin") {
  const lock = join(homedir(), "Library", "Caches", "loft-bun-test.lock");
  const O_EXLOCK = 0x20; // Darwin fcntl.h; node:fs does not expose this flag.
  try {
    openSync(lock, constants.O_CREAT | constants.O_RDWR | constants.O_NOFOLLOW | constants.O_NONBLOCK | O_EXLOCK, 0o600);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EAGAIN") throw error;
    console.error("LOFT tests busy on this Mac. Defer this check to the suite owner. Lock holder: lsof " + lock);
    process.exit(75);
  }
}
setDefaultTimeout(60_000);
