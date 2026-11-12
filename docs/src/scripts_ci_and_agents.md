# Using EIM from scripts and CI

Use the `eim` command. The GUI is for interactive use. When a command fails, the message names an `EIM-E` number. Open that section in [Troubleshooting](./troubleshooting.md). Installing a path that is already installed finishes successfully and does not reinstall it. Repair it with `eim fix`.

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

## Install a version

`eim install` does not ask questions unless you run `eim wizard`.

```bash
eim install -i v5.4.2
```

`--idf-versions` is the long name of `-i`. There is no `--idf-version`. Several versions are comma-separated.

`eim help-json` prints the command tree as JSON and does not start logging.

## Telemetry opt-out

```bash
eim install -i v5.4.2 --do-not-track true
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
| Offline `.zst` | `eim install --use-local-archive <file>.zst` | Do not unpack the `.zst`. Python for that archive is 3.11 to 3.14 |
| A failure you do not recognise | Open `eim.log` and the `EIM-E` number in [the guide](./troubleshooting.md) | |

## Mirrors

| Flag | Replaces |
|------|----------|
| `--mirror` | Tool download host (`github.com` asset URLs) |
| `--idf-mirror` | ESP-IDF git host |
| `--pypi-mirror` | Extra Python package index |

They do not change `dl.espressif.com` downloads of the version list or the constraints file ([EIM-E301](./troubleshooting.md#eim-e301), [EIM-E204](./troubleshooting.md#eim-e204)).
