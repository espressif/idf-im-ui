# Using EIM with git bisect

`git bisect` is useful when a regression appeared somewhere between two ESP-IDF revisions and you want git to find the exact commit for you. Bisecting ESP-IDF is a little more involved than bisecting an ordinary repository, because every revision you land on may require a different toolchain, different Python packages and different submodule commits. EIM can take care of all of that at every step.

This page describes the workflow: you own the Git repository, EIM owns the toolchain.

## Why you clone the repository yourself

When EIM downloads ESP-IDF, it performs a **shallow clone of a single reference** (the equivalent of `git clone --depth 1 --single-branch`). This keeps installations small and fast, but it means the repository contains only one commit and one branch. `git bisect` needs the history between your good and bad revisions, so it cannot work in a repository created this way.

For bisecting, start from your own full clone instead, then point EIM at it. EIM detects an existing ESP-IDF repository and installs the tools around it without downloading or modifying anything — see [Handling Existing ESP-IDF Repositories](./configuration.md#handling-existing-esp-idf-repositories).

## One-time setup

Clone ESP-IDF with its full history and check out your known-good revision:

```bash
git clone --recursive https://github.com/espressif/esp-idf.git ~/esp/esp-idf-bisect
cd ~/esp/esp-idf-bisect
git checkout v5.5.5
```

> **Tip:** A full ESP-IDF clone with submodules is several gigabytes. If you only need the history for bisecting, `git clone --filter=blob:none https://github.com/espressif/esp-idf.git ~/esp/esp-idf-bisect` is considerably faster — it fetches the complete commit history but downloads file contents on demand as you check revisions out. You still need `git submodule update --init --recursive` afterwards.

Install the toolchain with EIM, giving the installation a fixed name:

```bash
eim install -p ~/esp/esp-idf-bisect --version-name bisect
```

The `--version-name bisect` part is important; the next section explains why. Any ESP-IDF version you pass with `-i` is ignored here, because EIM uses the revision already checked out in your repository.

Finally, copy a small test project **outside** the repository. Building from inside the worktree means every `git checkout` that bisect performs interferes with your build directory:

```bash
mkdir -p ~/esp/bisect-proj
cp -R ~/esp/esp-idf-bisect/examples/get-started/hello_world/. ~/esp/bisect-proj/
```

## Why `--version-name` matters

Without `--version-name`, an installation that uses an existing repository is named after the **first 7 characters of the commit that was checked out when you installed** — even if that commit happens to be a release tag. So installing at `v6.0.2` gives you an installation called `7101770`.

The name is not just a label. It is part of several paths:

- the Python virtual environment: `<tool install folder>/python/<name>/venv`
- the activation and deactivation scripts: `activate_idf_<name>.sh`, `activate_idf_<name>.fish`, `Microsoft.<name>.PowerShell_profile.ps1`
- the identifier you pass to `eim select`, `eim run` and `eim remove`

The name is also **frozen at install time**. `eim fix` deliberately keeps it, whatever revision you have checked out, and `eim install` refuses to run a second time on a path that is already registered:

```
The IDF at path '/home/user/esp/esp-idf-bisect' is already installed.
If you need to reinstall or fix it, please use the 'fix' command.
```

Taken together, that means you will not accumulate one environment per commit — but it also means an unnamed installation keeps the hash of one arbitrary revision for the entire bisect. You have to remember that hash and type it into every `eim run`, and it stops matching the checkout after the very first step, which makes it actively misleading when you come back to the logs.

Passing `--version-name bisect` gives you a stable, meaningful handle instead, for a Python environment that is rebuilt in place as you move between revisions. You can confirm nothing is multiplying at any point during the bisect:

```bash
ls ~/.espressif/tools/python/    # one entry per installation; 'bisect' stays put
```

Toolchains are not affected by the name at all — they live in a shared directory keyed by tool name and version, so revisiting a revision whose compiler you already installed costs nothing.

## Bisecting, step by step

Start the bisect as you would in any repository:

```bash
cd ~/esp/esp-idf-bisect
git bisect start <bad-revision> <good-revision>
```

`git bisect` checks out a revision for you. Everything after that is yours to do, and it is the same four things at every step — re-sync the submodules, re-sync the environment, clear the build directory, build:

```bash
git submodule sync --recursive
git submodule update --init --recursive

eim fix -p ~/esp/esp-idf-bisect

rm -rf ~/esp/bisect-proj/build ~/esp/bisect-proj/sdkconfig
eim run "idf.py -C ~/esp/bisect-proj set-target esp32 && idf.py -C ~/esp/bisect-proj build" bisect
```

Then tell git what you saw, with `git bisect good` or `git bisect bad`, and repeat until it names the first bad commit. When you are finished, restore your original checkout:

```bash
git bisect reset
```

The rest of this section explains why each of those is there.

**Update the submodules.** Submodule commits are pinned per revision, and ESP-IDF adds, removes and moves submodules between versions. `git bisect` only moves the superproject, so the submodules have to be re-synced separately.

When a revision removes a submodule, `git checkout` prints warnings like `warning: unable to rmdir 'components/json/cJSON': Directory not empty`. These are expected while bisecting and do not affect the build — git is declining to delete a directory that still holds the old submodule's checkout.

**Run `eim fix`.** This re-reads `tools/tools.json` and `tools/requirements/*` from the worktree as it currently stands, installs any tool versions the new revision needs, and refreshes the Python environment against the constraints file matching the revision's `tools/cmake/version.cmake`. It never touches your repository — no fetch, no checkout, no submodule handling — and it keeps the name recorded at install time. It also runs without prompting, because it recovers the configuration stored with the installation, which is what makes it safe to call from a `git bisect run` script. See the [Fix command](./cli_commands.md#fix-command).

The existing Python virtual environment is kept rather than rebuilt, so pip installs only the packages the new revision needs and leaves everything already satisfying its constraints alone. Inside one release line that usually means nothing to do and no network traffic; when a revision tightens a constraint, the affected package is upgraded. This is what makes it affordable to run at every step. If pip fails, the environment is recreated once and retried — see [The Python environment](./cli_commands.md#the-python-environment).

`fix` is necessary because EIM's activation script is generated once, with the toolchain paths and environment variables written into it. Unlike ESP-IDF's own `export.sh`, which recomputes the environment every time you source it, the activation script cannot notice that the checked-out revision now needs a different compiler — it will simply put the previously installed one on `PATH`. `eim fix` is what regenerates it.

**Delete the build directory.** A CMake build directory produced by a different ESP-IDF version will fail in confusing ways, or worse, succeed against stale cached paths. Remove `build/` and `sdkconfig` whenever the version changes. Deleting `sdkconfig` is why `set-target` appears in the command above — a fresh `sdkconfig` has no target selected.

**Run the test.** `eim run "<command>" bisect` executes your command inside the activated environment of the `bisect` installation, so you do not have to source an activation script first. Adding `idf.py --version` to the command is worth considering: it prints a `git describe` string such as `ESP-IDF v6.0.1-700-g7fae4eace6`, which ties each step of your log to an exact commit and confirms the environment really did follow the checkout.

> **Note:** `git bisect start <bad> <good>` works best when the good revision is an ancestor of the bad one. Two tags from different release branches are not: `v5.5.4` is not an ancestor of `v6.0.2`, and `git rev-list --count v5.5.4..v6.0.2` is around 6400 commits, so git has to test roughly thirteen revisions rather than the handful the version numbers suggest. Bisecting across releases is perfectly valid and works — just check the size of the range with `git rev-list --count <good>..<bad>` before you start, so you know what you are committing to.

## Automating it with `git bisect run`

If your test is scriptable, wrap the four commands above in a shell script and hand it to `git bisect run` instead of answering good and bad by hand. One thing about EIM matters when you do:

`git bisect run` reads your script's exit code as `0` for good, `1` (or any other non-zero) for bad, and `125` for "this revision cannot be tested, skip it". `eim run` collapses every failure into exit code `1` and never produces `125` itself, so your wrapper has to decide. Return `125` yourself when the submodule update or `eim fix` fails, otherwise a broken environment is recorded as a code regression and the commit git blames at the end will be the wrong one.

Keep the same rule in mind when the build is skipped for legitimate reasons — a revision that predates the target you are building for, for instance, should be a `125`, not a `1`.

## Keeping it fast

Within a single release line you can often skip calling `eim fix` entirely unless the revision you just checked out actually touched something relevant. This tells you whether it did:

```bash
git diff --quiet <previous-revision> HEAD -- \
    tools/tools.json 'tools/requirements/*' tools/cmake/version.cmake
```

If that exits zero, nothing about the environment changed and you can go straight to the build. If you are bisecting entirely inside one release line, you can go further and skip EIM altogether between steps: source `activate_idf_bisect.sh` once and then just build at each revision.

Other things that help:

- Bisect the narrowest range you can. If you know the regression is inside one release line, use two patch tags from that line rather than two major versions.
- Build the smallest project that reproduces the problem, for a single target.
- Install `ccache` and export `IDF_CCACHE_ENABLE=1` before starting, which pays off whenever a revision is revisited.

## Things to watch out for

> **Warning:** Do not use `eim remove` to clean up a bisect installation. For an installation that uses an existing repository, the path EIM has recorded **is your clone**, and removing the installation deletes that directory — along with its parent directory if it ends up empty. Use `git bisect reset` and delete the test project by hand instead.

- **Do not run `git clean -xfd` in the repository** if you moved the tool install location inside the worktree by passing a relative `--tool-install-folder-name`. It would delete the toolchain and the Python environment along with the build artifacts. With the default (absolute) tool paths this is not a concern.
- **The installation name no longer tells you which revision is checked out.** That is the point of pinning it, but it means `eim list` will keep saying `bisect`. Use `idf.py --version`, or `git -C <repo> rev-parse --short HEAD`, to see where you actually are.
- **A stale environment fails quietly.** If you skip `eim fix` after a revision that needs a different toolchain, the build may fail for reasons that have nothing to do with the commit being tested. When a bisect result looks implausible, re-run the suspect revisions with `eim fix` forced.
