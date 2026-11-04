# Using EIM from scripts, CI and AI agents

EIM is a single `eim` binary. Scripts and agents should use the CLI. The GUI is for interactive use.

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | The command finished. |
| `1` | The command failed. |

There is no per-cause exit code. Read the text.

## How to read a failure

1. On failure, find the line that starts with `Error executing CLI:` ([EIM-E900](./troubleshooting.md#eim-e900)). The lines after it are the specific error's `Fix:` and `Docs:`, then the generic E900 block with the log location. The last stderr line is the E900 link. Do not use `tail -n1` as the specific ID.
2. In `eim.log`, `Fix:` and `Docs:` follow the `ERROR` or `WARN` line they belong to and have no timestamp prefix. Read from that line up to the next line that starts with a timestamp. `Fix:` and `Docs:` are the last two lines of that block. `grep -A2` misses them when git stderr, pip output, or PowerShell stderr is inserted first. `grep -A10` is a wider example and can still stop short. A `WARN` `Fix:` is conditional. Do not kill a run that is still going.
3. stdout prints `[FAIL]` for a failed check and `Hint:` on the next line with the step for that check.
4. The specific message keeps its original sentence and then adds `Fix:` and `Docs:`. Open the `EIM-E` ID on the [troubleshooting](./troubleshooting.md) page. Chinese text gives the ID only, unless the message already contains an English keyword such as `prerequisite` or `Python`, in which case it also carries the URL.
5. If the message has no ID, match the operating-system text. Disk full is [EIM-E501](./troubleshooting.md#eim-e501). A denied path is [EIM-E502](./troubleshooting.md#eim-e502).

## Logs and verbosity

Default CLI log: `<data dir>/eim/logs/eim.log`.

| Platform | Directory |
|----------|-----------|
| Linux | `$XDG_DATA_HOME/eim/logs` or `$HOME/.local/share/eim/logs` |
| macOS | `$HOME/Library/Application Support/eim/logs` |
| Windows | `%LOCALAPPDATA%\eim\logs` |

| File | Writer |
|------|--------|
| `eim.log` | CLI |
| `eim_gui.log` | GUI |
| `offline_installer.log` | Offline archive builder |

`--log-file <path>` replaces the CLI log path. If the data directory cannot be created, the CLI writes `eim.log` in the current directory.

`-v` raises stderr verbosity. Repeat it for more detail. The file log is already the most detailed level.

## Non-interactive install

```bash
eim install -i v5.4.2 --non-interactive true
```

`-n true` is the same flag. `--idf-versions` is the long name of `-i`. There is no `--idf-version`. Several versions are comma-separated.

`eim help-json` prints the command tree as JSON and does not start logging.

## Telemetry opt-out

```bash
eim install -i v5.4.2 --non-interactive true --do-not-track true
```

The environment variable `EIM_DO_NOT_TRACK=1` or `EIM_DO_NOT_TRACK=true` does the same. Other values do not.

## Prerequisites are not installed on Linux or macOS

`--install-all-prerequisites` / `-a` installs missing Git and Python on Windows only. On Linux and macOS, EIM prints the missing names and exits. Install those packages in the image, then rerun. See [EIM-E101](./troubleshooting.md#eim-e101).

`--skip-prerequisites-check true` skips the check. Use it only when the packages are already installed.

## Recovery

| What you see | What to run | What not to do |
|--------------|-------------|----------------|
| Missing packages on Linux or macOS | Install them with the distro package manager or Homebrew, then rerun the same `eim` command | Do not expect `-a` to install them. Do not use `--skip-prerequisites-check` in place of installing them |
| Missing Git or Python on Windows | `eim install --install-all-prerequisites true` | Do not ignore the package names in the output |
| `Python sanity check failed` and `[FAIL]` | Follow each `Hint:` line. Use Python 3.10 to 3.14 | Do not treat the summary line as the only instruction |
| `Download attempt` / `failed` on a `WARN` line | Nothing, while EIM is still retrying. If the command then exits 1: `--mirror` for tool assets, `--idf-mirror` for the ESP-IDF git host, `--pypi-mirror` for pip | Do not restart on the first attempt. Do not point all three flags at the same URL |
| `git clone failed` | `--idf-mirror https://git.espressif.com.cn` (or `https://github.com`) | Do not delete an existing checkout that you still need. A non-empty path is [EIM-E402](./troubleshooting.md#eim-e402) |
| `is already installed` | The install exits 0 and does nothing. Repair with `eim fix --path <path>`, or pass a new `--path` | Do not treat exit 0 as a finished install of that path |
| `No space left on device` | Export `TMPDIR` to a disk with room, or free space. See [EIM-E501](./troubleshooting.md#eim-e501) | Do not retry in a loop on a 1 GB `/tmp` |
| Offline `.zst` | `eim install --use-local-archive <file>.zst --non-interactive true` | Do not unpack the `.zst`. Python for that archive is 3.11 to 3.14 |
| `Error executing CLI:` and no earlier ID | Open `eim.log` and match [the guide](./troubleshooting.md) | Do not branch on exit code `1` alone |

## Mirrors

| Flag | Replaces |
|------|----------|
| `--mirror` | Tool download host (`github.com` asset URLs) |
| `--idf-mirror` | ESP-IDF git host |
| `--pypi-mirror` | Extra Python package index |

They do not change `dl.espressif.com` downloads of the version list or the constraints file ([EIM-E301](./troubleshooting.md#eim-e301), [EIM-E204](./troubleshooting.md#eim-e204)).
