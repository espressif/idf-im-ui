use anyhow::{anyhow, Result};
use gix::bstr::{BString, ByteSlice};
use gix::error::ErrorExt;
use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit};
use gix::refs::Target;
use log::{debug, error, info, trace, warn};
use std::fs::{self, read_to_string, OpenOptions};
use std::io::{BufRead, BufReader};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

use crate::command_executor::{execute_command_with_dir, spawn_with_dir};
use crate::ensure_path;
use std::io::Write;

/// Checks out a specific commit in a repository using the `git` command-line tool.
///
/// This function is a straightforward wrapper around `git checkout <commit_sha>`.
/// It is considered a fast and reliable operation.
///
/// # Arguments
///
/// * `dest_path` - The path to the local repository.
/// * `commit_sha` - The SHA of the commit to check out.
///
/// # Returns
///
/// * `Ok(())` if the checkout is successful.
/// * `Err` with a descriptive error message if the `git` command fails.
pub fn checkout_with_git_cli(
    dest_path: &Path,
    commit_sha: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    run_git_checked(
        &["checkout", commit_sha],
        dest_path.to_str().unwrap(),
        "git checkout",
    )
}

/// Runs `git <args>` in `dir` and turns a non-zero exit into `"<what> failed: <stderr>"`.
fn run_git_checked(args: &[&str], dir: &str, what: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = execute_command_with_dir("git", args, dir)?;
    if !output.status.success() {
        return Err(format!(
            "{} failed: {}",
            what,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

/// Represents messages for tracking the progress of Git operations.
///
/// This enum is used to send updates from long-running Git tasks (like cloning or fetching)
/// to another thread, typically for updating a user interface.
pub enum ProgressMessage {
    /// A general progress update. The value is a percentage (0-100).
    Update(u64),
    /// Indicates that the operation has completed successfully.
    Finish,
    /// A progress update for a specific submodule. The tuple contains the submodule name and its progress percentage (0-100).
    SubmoduleUpdate((String, u64)),
    /// Indicates that the processing of a specific submodule has finished. The string is the submodule's name.
    SubmoduleFinish(String),
}

/// Fetches a single commit from a remote repository using the `git` command-line tool.
///
/// This function serves as a fallback mechanism when a more direct method (like `gix`) fails.
/// It initializes a new repository if one doesn't exist, adds the remote, and then performs
/// a shallow fetch (`--depth 1`) for the specified commit. It parses the stderr of the `git`
/// process to provide progress updates.
///
/// # Arguments
///
/// * `dest_path` - The path to the local repository.
/// * `url` - The URL of the remote repository.
/// * `commit_sha` - The SHA of the commit to fetch.
/// * `tx` - An optional sender for sending `ProgressMessage` updates.
/// * `submodule_name` - An optional name of the submodule, used for submodule-specific progress updates.
///
/// # Returns
///
/// * `Ok(())` if the fetch and checkout are successful.
/// * `Err` if any of the `git` commands fail.
pub fn fetch_single_commit_git_cli(
    dest_path: &Path,
    url: &str,
    commit_sha: &str,
    tx: &Option<Sender<ProgressMessage>>,
    submodule_name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    fetch_commit_with_git_cli(dest_path, url, commit_sha, &|progress| {
        if let (Some(tx), Some(submodule_name)) = (tx, submodule_name) {
            let _ = tx.send(ProgressMessage::SubmoduleUpdate((
                submodule_name.to_string(),
                progress as u64,
            )));
        }
    })
}

/// Initializes `dest_path` if needed, shallow-fetches `commit_sha` (falling back to the
/// default branch) and checks it out, reporting milestones 10 / 10-80 / 80 / 100.
fn fetch_commit_with_git_cli(
    dest_path: &Path,
    url: &str,
    commit_sha: &str,
    report: &dyn Fn(u8),
) -> Result<(), Box<dyn std::error::Error>> {
    if !dest_path.join(".git").exists() {
        std::fs::create_dir_all(dest_path)?;
        let dir = dest_path.to_str().unwrap();
        run_git_checked(&["init"], dir, "git init")?;
        run_git_checked(&["remote", "add", "origin", url], dir, "git remote add")?;
    }

    // 10% - Initialized
    report(10);

    let mut child = spawn_with_dir(
        "git",
        &["fetch", "--depth", "1", "--progress", "origin", commit_sha],
        dest_path.to_str().unwrap(),
    )?;

    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(percentage) = parse_git_progress(&line) {
                report(scale_fetch_progress(percentage));
            }
        }
    }

    let status = child.wait()?;

    if !status.success() {
        debug!("Direct SHA fetch failed, trying default branch");
        run_git_checked(
            &["fetch", "--depth", "1", "origin"],
            dest_path.to_str().unwrap(),
            "git fetch",
        )?;
    }

    // 80% - Fetched
    report(80);

    checkout_with_git_cli(dest_path, commit_sha)?;

    // 100% - Complete
    report(100);

    Ok(())
}

/// Scales git's 0-100% fetch progress into the 10-80% band.
///
/// The arithmetic is done in `u8`, so inputs above 3 overflow (panic in debug builds,
/// wrap in release builds).
fn scale_fetch_progress(percentage: u64) -> u8 {
    10 + ((percentage as u8) * 70 / 100)
}

/// Parses a line of `git fetch` stderr output to extract a progress percentage.
///
/// Git's progress output can have a few different formats, such as:
/// - "Receiving objects:  45% (123/456)"
/// - "Resolving deltas: 100% (12/12), done."
///
/// This function attempts to parse the percentage value from these lines.
///
/// # Arguments
///
/// * `line` - A string slice representing a single line from git's stderr.
///
/// # Returns
///
/// * `Some(percentage)` if a percentage is successfully parsed.
/// * `None` if the line does not contain a recognizable progress format.
fn parse_git_progress(line: &str) -> Option<u64> {
    // Look for patterns like "Receiving objects:  45%" or "Resolving deltas:  12%"
    if let Some(pos) = line.find('(') {
        if let Some(end_pos) = line.find(')') {
            let content = &line[pos + 1..end_pos];
            if content.contains('%') {
                // Extract just the percentage number
                let percentage_str = content.replace("%", "");
                if let Ok(percentage) = percentage_str.trim().parse::<u64>() {
                    return Some(percentage);
                }
            }
        }
    }

    // Alternative pattern: "Receiving objects:  45% (123/456)"
    let percent_pos = line.find("Receiving objects: ")?;
    let percent_start = percent_pos + 19; // length of "Receiving objects: "
    let percent_part = &line[percent_start..];
    let end_pos = percent_part.find(' ')?;
    let percentage_str = &percent_part[..end_pos].replace("%", "");
    percentage_str.parse::<u64>().ok()
}

/// A helper function to send progress updates via an optional `Sender`.
///
/// It constructs the appropriate `ProgressMessage` based on whether a `submodule_name`
/// is provided and sends it through the channel.
///
/// # Arguments
///
/// * `tx` - An optional `Sender<ProgressMessage>`. If `None`, the function does nothing.
/// * `submodule_name` - An optional name of a submodule. If `Some`, a `SubmoduleUpdate` message is sent.
///   If `None`, a general `Update` message is sent.
/// * `percentage` - The progress percentage (0-100).
fn send_progress(
    tx: &Option<Sender<ProgressMessage>>,
    submodule_name: Option<&str>,
    percentage: u64,
) {
    if let Some(ref tx) = tx {
        let msg = match submodule_name {
            Some(name) => ProgressMessage::SubmoduleUpdate((name.to_string(), percentage)),
            None => ProgressMessage::Update(percentage),
        };
        let _ = tx.send(msg);
    }
}

/// Clones a Git repository using the `gix` library.
///
/// This function handles the entire process of cloning, including setting up a shallow clone,
/// fetching the repository data, checking out the main worktree, checking out a specific
/// reference (branch, tag, or commit), and recursively updating submodules if requested.
///
/// # Arguments
///
/// * `options` - A `CloneOptions` struct specifying the URL, local path, reference, and other clone settings.
/// * `tx` - A sender for reporting `ProgressMessage` updates.
///
/// # Returns
///
/// * `Ok(PathBuf)` with the path to the cloned repository on success.
/// * `Err` if any stage of the cloning process fails.
pub fn clone_repository(
    options: CloneOptions,
    tx: Sender<ProgressMessage>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dest_path = PathBuf::from(&options.path);

    let shallow = clone_shallow_mode(options.shallow, &options.reference);

    let should_interrupt = &AtomicBool::new(false);
    let progress = gix::progress::Discard;

    // Prepare clone - parse URL once, will be cloned inside retry closure
    let url = gix::url::parse(options.url.as_str()).map_err(gix::Error::from)?;

    info!("Cloning repository from {:?}", options);

    // Retry with fresh connection handle each time to avoid hitting same CDN cache
    let mut attempt = 0;
    let fetch_result = crate::utils::with_retry_exponential(
        || {
            attempt += 1;
            if attempt > 1 {
                // Only clean up on actual retries, not first attempt
                let _ = std::fs::remove_dir_all(&dest_path);
            }
            std::fs::create_dir_all(&dest_path).ok();
            clone_fetch_attempt(
                &url,
                &dest_path,
                &shallow,
                &options.reference,
                should_interrupt,
            )
        },
        3,                                     // max_retries
        std::time::Duration::from_millis(500), // base_delay
    );

    let (mut checkout, _) = match fetch_result {
        Ok(res) => res,
        Err(e) => {
            let _ = tx.send(ProgressMessage::Finish);
            error!("Failed to fetch repository: {}", e);
            return Err(anyhow::anyhow!("{}", e).into());
        }
    };

    let _ = tx.send(ProgressMessage::Update(50));

    // Checkout
    let (repo, _) = match checkout.main_worktree(progress, should_interrupt) {
        Ok(res) => res,
        Err(e) => {
            let _ = tx.send(ProgressMessage::Finish);
            error!("Failed to checkout repository: {}", e);
            return Err(Box::new(e));
        }
    };

    // On Windows, disable symlinks so Git does not report typechanges
    #[cfg(windows)]
    {
        let config_path = repo.git_dir().join("config");
        if let Ok(config) = fs::read_to_string(&config_path) {
            if let Some(config) = config_with_symlinks_disabled(&config) {
                let _ = fs::write(&config_path, config);
            }
        }
    }

    // Checkout specific reference
    match checkout_reference(&repo, &options.reference) {
        Ok(_) => {
            let _ = tx.send(ProgressMessage::Update(90));
        }
        Err(e) => {
            let _ = tx.send(ProgressMessage::Finish);
            error!("Failed to checkout reference: {}", e);
            return Err(anyhow!("Failed to checkout reference: {}", e).into());
        }
    }

    info!(
        "Cloned repository to {} proceeding to submodules...",
        dest_path.display()
    );

    // Handle submodules
    info!("Starting submodule update...");
    if options.recurse_submodules {
        info!("Recurse submodules is TRUE");
        match update_submodules_shallow(&repo, tx.clone(), options.mirror.as_deref()) {
            Ok(_) => info!("Submodules updated successfully"),
            Err(e) => {
                let _ = tx.send(ProgressMessage::Finish);
                error!("Submodule update failed: {}", e);
                return Err(anyhow!("Submodule update failed: {}", e).into());
            }
        }
    }

    let _ = tx.send(ProgressMessage::Finish);
    Ok(dest_path)
}

/// Shallow mode for `clone_repository`: depth 1 unless a raw commit is requested,
/// since the commit may not be reachable from a depth-1 remote tip.
fn clone_shallow_mode(shallow: bool, reference: &GitReference) -> gix::remote::fetch::Shallow {
    match (shallow, reference) {
        (true, GitReference::Commit(_)) | (false, _) => gix::remote::fetch::Shallow::NoChange,
        (true, _) => gix::remote::fetch::Shallow::DepthAtRemote(NonZeroU32::new(1).unwrap()),
    }
}

/// Fetch refspec that limits a clone to the requested branch or tag.
fn clone_fetch_refspec(reference: &GitReference) -> Option<String> {
    match reference {
        GitReference::Branch(branch) => Some(format!(
            "+refs/heads/{}:refs/remotes/origin/{}",
            branch, branch
        )),
        GitReference::Tag(tag) => Some(format!("+refs/tags/{}:refs/tags/{}", tag, tag)),
        _ => None,
    }
}

/// One `gix` clone attempt with a fresh clone handle (and therefore a new connection).
fn clone_fetch_attempt(
    url: &gix::Url,
    dest_path: &Path,
    shallow: &gix::remote::fetch::Shallow,
    reference: &GitReference,
    should_interrupt: &AtomicBool,
) -> Result<(gix::clone::PrepareCheckout, gix::remote::fetch::Outcome), String> {
    let fresh_prepare = gix::prepare_clone(url.clone(), dest_path)
        .map_err(|e| format!("Failed to prepare clone: {}", e))?
        .with_remote_name("origin")
        .map_err(|e| format!("Failed to set remote name: {}", e))?
        .with_shallow(shallow.clone());

    let mut configured_prepare = match clone_fetch_refspec(reference) {
        Some(refspec) => fresh_prepare.configure_remote(move |remote| {
            remote
                .with_refspecs(
                    Some(BString::from(refspec.clone())),
                    gix::remote::Direction::Fetch,
                )
                .map_err(ErrorExt::raise_erased)
        }),
        None => fresh_prepare,
    };

    configured_prepare
        .fetch_then_checkout(gix::progress::Discard, should_interrupt)
        .map_err(|e| format!("Failed to fetch: {}", e))
}

/// Inserts `line` right after the `[core]` header line. Returns `false` if there is no
/// `[core]` header followed by a newline.
fn insert_after_core_section(config: &mut String, line: &str) -> bool {
    if let Some(pos) = config.find("[core]") {
        if let Some(end_pos) = config[pos..].find('\n') {
            config.insert_str(pos + end_pos + 1, line);
            return true;
        }
    }
    false
}

/// Returns the config with `symlinks = false`, or `None` if nothing needs to change
/// (already set to something other than `true`, or no `[core]` section to add it to).
#[cfg(any(windows, test))]
fn config_with_symlinks_disabled(config: &str) -> Option<String> {
    if config.contains("symlinks = true") {
        return Some(config.replace("symlinks = true", "symlinks = false"));
    }
    if config.contains("symlinks") {
        return None;
    }
    let mut updated = config.to_string();
    insert_after_core_section(&mut updated, "\tsymlinks = false\n").then_some(updated)
}

/// Updates all submodules in a repository to their specified commits using a shallow fetch.
///
/// This function manually implements the logic of `git submodule update --init`. It reads the
/// `.gitmodules` file, finds the commit SHA for each submodule in the parent repository's tree,
/// and then fetches only that specific commit for the submodule. This is more efficient than
/// cloning the entire history of each submodule. It handles nested submodules recursively.
///
/// # Arguments
///
/// * `repo` - The parent `gix::Repository` containing the submodules.
/// * `tx` - A sender for reporting `ProgressMessage` updates for each submodule.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err` if reading submodule configuration or updating a submodule fails.
pub fn update_submodules_shallow(
    repo: &gix::Repository,
    tx: Sender<ProgressMessage>,
    mirror: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let workdir = repo
        .workdir()
        .ok_or("Repository has no working directory")?;

    let gitmodules_path = workdir.join(".gitmodules");
    if !gitmodules_path.exists() {
        debug!("No .gitmodules file found, skipping submodule initialization");
        return Ok(());
    }

    debug!(".gitmodules found at {}", gitmodules_path.display());

    let submodules = match repo.submodules()? {
        Some(subs) => subs,
        None => {
            debug!("No submodules configured");
            return Ok(());
        }
    };

    let git_dir = repo.git_dir();

    // Resolve relative submodule URLs against the logical GitHub parent URL,
    // then apply the mirror prefix — equivalent to git's url.insteadOf.
    let parent_url = get_remote_url(repo)?;
    let resolution_base = reverse_github_mirror(&parent_url, mirror);
    debug!(
        "Parent repository URL: {} (resolution base: {})",
        parent_url, resolution_base
    );

    // Get HEAD tree to find submodule commit SHAs
    let head_commit = repo.head_commit()?;
    let tree = head_commit.tree()?;

    // Build a map of submodule paths to their commit OIDs
    let mut submodule_commits = std::collections::HashMap::new();
    collect_submodule_commits(&tree, "", &mut submodule_commits)?;

    debug!(
        "Found {} submodule commit entries in tree",
        submodule_commits.len()
    );

    for submodule in submodules {
        let name = submodule.name().to_string();
        let path = submodule.path().map_err(gix::Error::from)?.to_string();
        // Normalize path to forward slashes for consistency
        let normalized_path = path.replace('\\', "/");
        let url_raw = submodule.url()?.to_bstring().to_string();

        // Resolve relative URLs against GitHub, then rewrite to the mirror.
        let url = resolve_submodule_url(&url_raw, &resolution_base)?;
        let url = apply_github_mirror(&url, mirror);

        debug!("Processing submodule: {} at path: {}", name, path);
        if url != url_raw {
            debug!("  Resolved URL: {} -> {}", url_raw, url);
        }

        let _ = tx.send(ProgressMessage::SubmoduleUpdate((name.clone(), 0)));

        // Get the expected commit SHA
        let expected_sha = match submodule_commits.get(&normalized_path) {
            Some(oid) => oid.to_string(),
            None => {
                warn!(
                    "No commit entry found for submodule {} at path {}",
                    name, path
                );
                let _ = tx.send(ProgressMessage::SubmoduleFinish(name.clone()));
                continue;
            }
        };

        debug!(
            "Submodule {} should be at commit {}",
            name,
            &expected_sha[..7]
        );

        let submodule_dir = workdir.join(&path);
        let modules_dir = git_dir.join("modules").join(&path);

        // Step 1: Add to .git/config
        add_submodule_to_config(&git_dir.join("config"), &name, &path, &url)?;

        // Step 2: Initialize .git/modules/<path>
        initialize_modules_repo(&modules_dir, &submodule_dir, &url)?;

        // Step 3: Create submodule workdir and gitlink files
        std::fs::create_dir_all(&submodule_dir)?;
        create_gitlink(&submodule_dir, git_dir, &path)?;

        // Step 4: Fetch commit into modules dir
        fetch_single_commit_to_modules(
            &modules_dir,
            &url,
            &expected_sha,
            Some(tx.clone()),
            Some(&name),
        )?;

        // Step 5: Checkout files to workdir
        checkout_submodule_worktree(&modules_dir, &submodule_dir, &expected_sha)?;

        info!("✓ Submodule complete: {}", name);
        let _ = tx.send(ProgressMessage::SubmoduleFinish(name.clone()));

        // Recursively handle nested submodules
        if let Ok(sub_repo) = gix::open(&submodule_dir) {
            let _ = update_submodules_shallow(&sub_repo, tx.clone(), mirror);
        }
    }

    Ok(())
}

/// Retrieves the fetch URL of a remote for a `gix` repository.
///
/// It first attempts to find the remote named "origin". If that fails, it iterates
/// through all available remotes and returns the URL of the first one it finds.
///
/// # Arguments
///
/// * `repo` - The `gix::Repository` to inspect.
///
/// # Returns
///
/// * `Ok(String)` with the remote's fetch URL.
/// * `Err` if no remotes with a fetch URL can be found.
fn get_remote_url(repo: &gix::Repository) -> Result<String, Box<dyn std::error::Error>> {
    // Try to get the origin remote first
    match repo.find_remote("origin") {
        Ok(remote) => {
            let url = remote
                .url(gix::remote::Direction::Fetch)
                .ok_or("Origin remote has no fetch URL")?;
            Ok(url.to_bstring().to_string())
        }
        Err(_) => {
            // If origin doesn't exist, try to get any remote
            let remotes = repo.remote_names();

            for name in remotes.iter() {
                // Convert Cow<BStr> to &str
                if let Ok(name_str) = name.to_str() {
                    if let Ok(remote) = repo.find_remote(name_str) {
                        if let Some(url) = remote.url(gix::remote::Direction::Fetch) {
                            return Ok(url.to_bstring().to_string());
                        }
                    }
                }
            }

            Err("No remotes with fetch URLs found".into())
        }
    }
}

/// Resolves a potentially relative submodule URL against its parent repository's URL.
///
/// Submodule URLs in `.gitmodules` can be relative (e.g., `../another-repo.git`). This
/// function correctly resolves these relative URLs into absolute ones based on the parent
/// repository's remote URL. It handles both HTTP(S) and SCP-style SSH URLs.
///
/// # Arguments
///
/// * `submodule_url` - The URL of the submodule, which may be relative.
/// * `parent_url` - The absolute URL of the parent repository.
///
/// # Returns
///
/// * `Ok(String)` with the resolved, absolute URL for the submodule.
/// * `Err` if URL parsing fails.
fn resolve_submodule_url(
    submodule_url: &str,
    parent_url: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // If the URL starts with ./ or ../, it's relative
    if submodule_url.starts_with("./") || submodule_url.starts_with("../") {
        // Parse parent URL
        if parent_url.starts_with("http://") || parent_url.starts_with("https://") {
            // HTTP(S) URL - resolve relative to the path component
            resolve_http_relative_url(submodule_url, parent_url)
        } else if parent_url.contains(':') && !parent_url.starts_with('/') {
            // SSH URL like git@github.com:user/repo.git
            resolve_ssh_relative_url(submodule_url, parent_url)
        } else {
            // File path or other format
            Ok(submodule_url.to_string())
        }
    } else {
        // Absolute URL
        Ok(submodule_url.to_string())
    }
}

/// Resolves a relative URL path against a base HTTP(S) URL.
///
/// # Arguments
///
/// * `relative` - The relative path (e.g., `../foo.git`).
/// * `base` - The full base URL (e.g., `https://example.com/bar/baz.git`).
///
/// # Returns
///
/// * `Ok(String)` with the resolved URL (e.g., `https://example.com/foo.git`).
/// * `Err` if the base URL cannot be parsed.
fn resolve_http_relative_url(
    relative: &str,
    base: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Parse the base URL
    let base_url = url::Url::parse(base).map_err(|e| format!("Failed to parse base URL: {}", e))?;

    // Get the path without the .git extension and repo name
    let path_segments = resolve_relative_segments(
        base_url
            .path_segments()
            .ok_or("Base URL has no path")?
            .collect(),
        relative,
    );

    // Reconstruct URL
    let scheme = base_url.scheme();
    let host = base_url.host_str().ok_or("Base URL has no host")?;
    let path = path_segments.join("/");

    Ok(format!("{}://{}/{}", scheme, host, path))
}

/// Resolves a relative URL path against a base SCP-style SSH URL.
///
/// # Arguments
///
/// * `relative` - The relative path (e.g., `../foo.git`).
/// * `base` - The full base SSH URL (e.g., `git@example.com:bar/baz.git`).
///
/// # Returns
///
/// * `Ok(String)` with the resolved URL (e.g., `git@example.com:bar/foo.git`).
/// * `Err` if the base URL format is invalid.
fn resolve_ssh_relative_url(
    relative: &str,
    base: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Split into user@host and path
    let parts: Vec<&str> = base.split(':').collect();
    if parts.len() != 2 {
        return Err("Invalid SSH URL format".into());
    }

    let host_part = parts[0]; // e.g., "git@github.com"
    let path_part = parts[1]; // e.g., "user/repo.git"

    // Remove .git extension and split path
    let path_clean = path_part.trim_end_matches(".git");
    let path_segments = resolve_relative_segments(path_clean.split('/').collect(), relative);

    let new_path = path_segments.join("/");
    Ok(format!("{}:{}", host_part, new_path))
}

/// Drops the repository name (last segment) from `base`, then applies `relative`
/// segment by segment (`.` and empty segments are skipped, `..` pops).
fn resolve_relative_segments<'a>(mut base: Vec<&'a str>, relative: &'a str) -> Vec<&'a str> {
    base.pop();
    for segment in relative.split('/') {
        match segment {
            "." | "" => continue,
            ".." => {
                base.pop();
            }
            s => base.push(s),
        }
    }
    base
}

/// Recursively traverses a git tree to find all submodule entries (gitlinks) and their commit SHAs.
///
/// This function walks through a `gix::Tree`, identifying entries that are gitlinks (submodule
/// references). For each one found, it adds the submodule's full path and its corresponding
/// commit `ObjectId` to the provided HashMap. It descends into subtrees to find nested submodules.
///
/// # Arguments
///
/// * `tree` - The `gix::Tree` to search within.
/// * `prefix` - The path prefix for the current tree, used to construct the full path of entries.
/// * `commits` - A mutable HashMap to populate with `(path, ObjectId)` pairs for each submodule found.
///
/// # Returns
///
/// * `Ok(())` on successful traversal.
/// * `Err` if there is an issue iterating through the tree or its entries.
fn collect_submodule_commits(
    tree: &gix::Tree,
    prefix: &str,
    commits: &mut std::collections::HashMap<String, gix::ObjectId>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in tree.iter() {
        let entry = entry.map_err(gix::Error::from)?;
        let entry_mode = entry.mode();
        let entry_name = entry.filename();
        let entry_oid = entry.oid();

        // Build the full path
        let full_path = if prefix.is_empty() {
            entry_name.to_str_lossy().to_string()
        } else {
            format!("{}/{}", prefix, entry_name.to_str_lossy())
        };

        if entry_mode.is_commit() {
            // This is a gitlink (submodule reference)
            info!("Found submodule gitlink: {} -> {}", full_path, entry_oid);
            commits.insert(full_path.clone(), entry_oid.into());
        } else if entry_mode.is_tree() {
            // Recurse into subdirectories to find nested submodules
            let subtree = tree.repo.find_tree(entry_oid)?;
            collect_submodule_commits(&subtree, &full_path, commits)?;
        }
    }

    Ok(())
}

/// Initializes the repository structure for a submodule within the parent's `.git/modules/` directory.
///
/// This function performs the setup required to manage a submodule. It creates a bare repository
/// in `.git/modules/<path>`, then modifies its configuration to be non-bare, point its worktree
/// to the correct submodule working directory, and adds the remote "origin".
///
/// # Arguments
///
/// * `modules_dir` - The path to the submodule's repository inside `.git/modules/`.
/// * `submodule_workdir` - The path to the submodule's working directory.
/// * `url` - The remote URL of the submodule.
///
/// # Returns
///
/// * `Ok(())` on successful initialization.
/// * `Err` if directory creation or file I/O fails.
fn initialize_modules_repo(
    modules_dir: &Path,
    submodule_workdir: &Path,
    url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if already initialized
    if modules_dir.join("config").exists() {
        debug!(
            "Modules repo already initialized at {}",
            modules_dir.display()
        );
        return Ok(());
    }

    create_modules_dir(modules_dir)?;

    // Check what's in the directory before init
    info!(
        "Contents of {} before init: {:?}",
        modules_dir.display(),
        fs::read_dir(modules_dir)?.collect::<Vec<_>>()
    );

    let repo = open_or_init_bare(modules_dir)?;

    // Use the repository's git_dir (might be different from modules_dir)
    let git_dir = repo.git_dir();
    info!("Git dir is at: {}", git_dir.display());

    let config_path = git_dir.join("config");

    if !config_path.exists() {
        return Err(format!("Config file not found at {}", config_path.display()).into());
    }

    fs::create_dir_all(submodule_workdir)?;
    let rel_worktree = relative_worktree_path(modules_dir, submodule_workdir)?;

    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config at {}: {}", config_path.display(), e))?;
    let config_content = submodule_repo_config(config_content, &rel_worktree, url);

    fs::write(&config_path, config_content)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    info!("✓ Initialized modules repo: {}", modules_dir.display());
    Ok(())
}

fn create_modules_dir(modules_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Ensure parent directory exists
    if let Some(parent) = modules_dir.parent() {
        info!("Creating parent directories: {}", parent.display());
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent dir {}: {}", parent.display(), e))?;
    }

    // Create modules_dir itself if it doesn't exist
    if !modules_dir.exists() {
        info!("Creating modules dir: {}", modules_dir.display());
        fs::create_dir_all(modules_dir).map_err(|e| {
            format!(
                "Failed to create modules dir {}: {}",
                modules_dir.display(),
                e
            )
        })?;
    }
    Ok(())
}

/// Opens the repository at `modules_dir`, or initializes a bare one if it can't be opened.
fn open_or_init_bare(modules_dir: &Path) -> Result<gix::Repository, Box<dyn std::error::Error>> {
    match gix::open(modules_dir) {
        Ok(repo) => {
            info!("Repository already exists at {}", modules_dir.display());
            Ok(repo)
        }
        Err(_) => {
            info!("Initializing new git repo at {}", modules_dir.display());
            Ok(gix::init_bare(modules_dir).map_err(|e| {
                format!(
                    "Failed to init git repo at {}: {}",
                    modules_dir.display(),
                    e
                )
            })?)
        }
    }
}

/// Relative path from `modules_dir` to the submodule worktree (matches standard Git behavior).
fn relative_worktree_path(
    modules_dir: &Path,
    submodule_workdir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let modules_canon = modules_dir
        .canonicalize()
        .unwrap_or_else(|_| modules_dir.to_path_buf());
    let workdir_canon = submodule_workdir
        .canonicalize()
        .unwrap_or_else(|_| submodule_workdir.to_path_buf());
    Ok(pathdiff::diff_paths(&workdir_canon, &modules_canon)
        .ok_or("Failed to compute relative worktree path")?)
}

/// Turns the bare-repo config into a non-bare submodule config: sets `bare = false`, adds
/// `worktree` and the `origin` remote (and `symlinks = false` on Windows) unless present.
fn submodule_repo_config(mut config_content: String, rel_worktree: &Path, url: &str) -> String {
    if config_content.contains("bare = true") {
        config_content = config_content.replace("bare = true", "bare = false");
    }

    if !config_content.contains("worktree") {
        insert_after_core_section(
            &mut config_content,
            &format!(
                "\tworktree = {}\n",
                rel_worktree.display().to_string().replace('\\', "/")
            ),
        );
    }

    #[cfg(windows)]
    if let Some(updated) = config_with_symlinks_disabled(&config_content) {
        config_content = updated;
    }

    if !config_content.contains("[remote \"origin\"]") {
        config_content.push_str(&format!(
            "\n[remote \"origin\"]\n\
             \turl = {}\n\
             \tfetch = +refs/heads/*:refs/remotes/origin/*\n",
            url
        ));
    }

    config_content
}

/// Creates the `.git` file in a submodule's working directory.
///
/// This file, often called a "gitlink," doesn't contain the repository itself but
/// instead points to the actual Git directory located within the parent's `.git/modules/`
/// directory. This function also creates the reverse link (`gitdir` file) in the modules
/// directory, which points back to the worktree.
///
/// # Arguments
///
/// * `submodule_dir` - The path to the submodule's working directory.
/// * `parent_git_dir` - The path to the parent repository's `.git` directory.
/// * `submodule_path` - The relative path of the submodule within the parent repository.
///
/// # Returns
///
/// * `Ok(())` on successful creation of the gitlink files.
/// * `Err` if file I/O fails.
fn create_gitlink(
    submodule_dir: &Path,
    parent_git_dir: &Path,
    submodule_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let gitlink_path = submodule_dir.join(".git");

    let normalized_path = submodule_path.replace('\\', "/");

    // modules dir is relative to the *parent git dir*.
    // For nested submodules, parent_git_dir will be something like:
    //   <root>/.git/modules/components/cmock/CMock
    // so nested modules live under:
    //   <root>/.git/modules/components/cmock/CMock/modules/vendor/c_exception
    let modules_dir = parent_git_dir.join("modules").join(&normalized_path);

    fs::create_dir_all(&modules_dir)?;

    // Canonicalize so diff_paths works reliably across symlinks / .. segments
    let modules_abs = modules_dir
        .canonicalize()
        .unwrap_or_else(|_| modules_dir.clone());
    let workdir_abs = submodule_dir
        .canonicalize()
        .unwrap_or_else(|_| submodule_dir.to_path_buf());

    // .git file: relative path from submodule worktree -> modules dir
    let rel_to_modules = pathdiff::diff_paths(&modules_abs, &workdir_abs)
        .ok_or("Failed to compute relative path from worktree to modules dir")?;
    let gitlink_content = format!(
        "gitdir: {}\n",
        rel_to_modules.display().to_string().replace('\\', "/")
    );
    fs::write(&gitlink_path, &gitlink_content)?;
    debug!(
        "Created gitlink at {}: {}",
        gitlink_path.display(),
        gitlink_content.trim()
    );

    // Reverse link: relative path from modules dir -> submodule worktree
    let rel_to_workdir = pathdiff::diff_paths(&workdir_abs, &modules_abs)
        .ok_or("Failed to compute relative path from modules dir to worktree")?;
    fs::write(
        modules_dir.join("gitdir"),
        format!(
            "{}\n",
            rel_to_workdir.display().to_string().replace('\\', "/")
        ),
    )?;

    Ok(())
}

/// Fetches a single commit into a submodule's repository located in `.git/modules/`.
///
/// This function uses `gix` to perform a shallow fetch (`depth=1`) of exactly the commit
/// required. If the commit already exists locally, the fetch is skipped. After a successful
/// fetch, it updates the `HEAD` of the submodule's repository to point to the fetched commit.
///
/// # Arguments
///
/// * `modules_dir` - The path to the submodule's repository inside `.git/modules/`.
/// * `url` - The remote URL of the submodule.
/// * `commit_sha` - The SHA of the commit to fetch.
/// * `tx` - An optional sender for reporting progress.
/// * `submodule_name` - An optional name of the submodule for progress reporting.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err` if the fetch fails or the commit cannot be found after fetching.
fn fetch_single_commit_to_modules(
    modules_dir: &Path,
    url: &str,
    commit_sha: &str,
    tx: Option<Sender<ProgressMessage>>,
    submodule_name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    send_progress(&tx, submodule_name, 20);

    // Open the repository at modules dir
    let repo = match gix::open(modules_dir) {
        Ok(repo) => repo,
        Err(_) => gix::init(modules_dir)?,
    };

    let expected_oid = parse_commit_oid(commit_sha)?;

    // Check if we already have this commit
    if repo.find_commit(expected_oid).is_ok() {
        debug!("Commit {} already exists", &commit_sha[..7]);
        send_progress(&tx, submodule_name, 70);

        // Update HEAD to point to this commit
        std::fs::write(modules_dir.join("HEAD"), format!("{}\n", commit_sha))?;
        return Ok(());
    }

    send_progress(&tx, submodule_name, 30);

    // Parse URL and create remote
    let remote_url = gix::url::parse(url).map_err(|e| format!("Invalid URL '{}': {}", url, e))?;

    send_progress(&tx, submodule_name, 40);

    let _outcome =
        fetch_commit_shallow_with_retry(&repo, &remote_url, commit_sha, &AtomicBool::new(false))
            .map_err(|e| {
                format!(
                    "fetch_single_commit_to_modules failed after 3 attempts: {}",
                    e
                )
            })?;

    send_progress(&tx, submodule_name, 70);

    // Verify commit exists
    repo.find_commit(expected_oid)
        .map_err(|_| format!("Commit {} not found after fetch", &commit_sha[..7]))?;

    // Update HEAD to point to this commit (detached state)
    std::fs::write(modules_dir.join("HEAD"), format!("{}\n", commit_sha))?;

    debug!("Fetched commit {} to modules dir", &commit_sha[..7]);
    Ok(())
}

fn parse_commit_oid(commit_sha: &str) -> Result<gix::ObjectId, Box<dyn std::error::Error>> {
    gix::ObjectId::from_hex(commit_sha.as_bytes()).map_err(|e| -> Box<dyn std::error::Error> {
        Box::new(std::io::Error::other(format!(
            "Invalid SHA '{commit_sha}': {e}"
        )))
    })
}

/// First 7 characters of a commit SHA (or the whole string if shorter), for log messages.
fn short_sha(commit_sha: &str) -> &str {
    &commit_sha[..7.min(commit_sha.len())]
}

/// Fetches exactly `commit_sha` at depth 1 without tags, retrying with a fresh remote
/// connection each time to avoid hitting the same CDN cache.
fn fetch_commit_shallow_with_retry(
    repo: &gix::Repository,
    remote_url: &gix::Url,
    commit_sha: &str,
    should_interrupt: &AtomicBool,
) -> Result<gix::remote::fetch::Outcome, String> {
    let shallow = gix::remote::fetch::Shallow::DepthAtRemote(NonZeroU32::new(1).unwrap());
    crate::utils::with_retry_exponential(
        || {
            repo.remote_at(remote_url.clone())
                .map_err(|e| format!("Failed to create remote: {}", e))?
                .with_fetch_tags(gix::remote::fetch::Tags::None)
                .with_refspecs([commit_sha], gix::remote::Direction::Fetch)
                .map_err(|e| format!("Failed to set refspec: {}", e))?
                .connect(gix::remote::Direction::Fetch)
                .map_err(|e| format!("Failed to connect: {}", e))?
                .prepare_fetch(
                    gix::progress::Discard,
                    gix::remote::ref_map::Options::default(),
                )
                .map_err(|e| format!("Failed to prepare fetch: {}", e))?
                .with_shallow(shallow.clone())
                .receive(gix::progress::Discard, should_interrupt)
                .map_err(|e| format!("Failed to receive: {}", e))
        },
        3,
        std::time::Duration::from_millis(500),
    )
}

/// Checks out the files from a submodule's repository into its working directory.
///
/// This function takes the commit from the repository stored in `.git/modules/` and
/// populates the submodule's working directory with the files from that commit's tree.
///
/// # Arguments
///
/// * `modules_dir` - The path to the submodule's repository inside `.git/modules/`.
/// * `submodule_workdir` - The path to the submodule's working directory where files will be checked out.
/// * `commit_sha` - The SHA of the commit to check out.
///
/// # Returns
///
/// * `Ok(())` on successful checkout.
/// * `Err` if the repository cannot be opened or the checkout process fails.
fn checkout_submodule_worktree(
    modules_dir: &Path,
    submodule_workdir: &Path,
    commit_sha: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Open the repository at modules dir
    let repo = gix::open(modules_dir)?;

    let commit_oid = gix::ObjectId::from_hex(commit_sha.as_bytes()).map_err(gix::Error::from)?;
    let commit = repo.find_commit(commit_oid)?;
    let tree = commit.tree()?;

    // Simple approach: Just checkout files, don't worry about index for now
    // Git will rebuild it when needed
    checkout_tree_recursive(&repo, &tree, submodule_workdir)?;

    debug!("Checked out files to {}", submodule_workdir.display());

    // Need this to prevent untracked files in the submodule workdir
    populate_index_from_tree(&repo, &tree)?;

    Ok(())
}

/// Recursively checks out the contents of a `gix::Tree` to a target directory.
///
/// This helper function iterates through a tree's entries. For subtrees, it creates a
/// corresponding directory and recurses. For blobs, it writes the file content to the
/// target directory. It also sets the executable bit for files where required on Unix-like systems.
///
/// # Arguments
///
/// * `repo` - The `gix::Repository` that owns the tree.
/// * `tree` - The `gix::Tree` to check out.
/// * `target_dir` - The directory where the tree's contents will be placed.
///
/// # Returns
///
/// * `Ok(())` on successful checkout.
/// * `Err` if file or directory I/O fails.
fn checkout_tree_recursive(
    repo: &gix::Repository,
    tree: &gix::Tree,
    target_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in tree.iter() {
        let entry = entry.map_err(gix::Error::from)?;
        let entry_mode = entry.mode();
        let entry_oid = entry.oid();
        let entry_name = entry.filename();

        let target_path = target_dir.join(entry_name.to_path_lossy().as_ref());

        if entry_mode.is_tree() {
            debug!("Tree entry: {}", entry_name);
            // Create directory and recurse
            fs::create_dir_all(&target_path)?;

            let subtree = repo.find_tree(entry_oid)?;
            checkout_tree_recursive(repo, &subtree, &target_path)?;
        } else if entry_mode.is_commit() {
            debug!("Commit entry: {}", entry_name);
            // gitlink (submodule): materialize as a directory placeholder
            // so the superproject doesnot see it as deleted
            fs::create_dir_all(&target_path)?;
        } else if entry_mode.is_link() {
            debug!("Symlink entry: {}", entry_name);
            // Symlink entries store their target path in blob content.
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            write_symlink_from_blob(repo, entry.oid().into(), &target_path)?;
        } else if entry_mode.is_blob() || entry_mode.is_executable() {
            debug!("Blob entry: {}", entry_name);
            // Ensure parent directory exists
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Write blob content
            let object = repo.find_object(entry.oid())?;
            let blob = object.try_into_blob().map_err(gix::Error::from)?;
            fs::write(&target_path, blob.data.clone())?;

            // Set executable bit if needed
            #[cfg(unix)]
            if entry_mode.is_executable() {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&target_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&target_path, perms)?;
            }
        }
    }

    Ok(())
}

fn write_symlink_from_blob(
    repo: &gix::Repository,
    oid: gix::ObjectId,
    target_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let object = repo.find_object(oid)?;
    let blob = object.try_into_blob().map_err(gix::Error::from)?;
    let link_target = std::str::from_utf8(blob.data.as_ref())
        .map_err(|e| {
            format!(
                "Invalid UTF-8 in symlink target for {}: {}",
                target_path.display(),
                e
            )
        })?
        .trim_end_matches('\n');

    if target_path.exists() {
        if target_path.is_dir() {
            fs::remove_dir_all(target_path)?;
        } else {
            fs::remove_file(target_path)?;
        }
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(link_target, target_path)?;
    }

    #[cfg(not(unix))]
    {
        // Fallback for platforms without standard symlink support in this path.
        fs::write(target_path, link_target.as_bytes())?;
    }

    Ok(())
}

/// Adds a submodule's configuration to the parent repository's `.git/config` file.
///
/// This function appends a `[submodule "<name>"]` section with the submodule's path and URL
/// to the main `.git/config` file. It checks if the section already exists to avoid duplicates.
///
/// # Arguments
///
/// * `config_path` - The path to the parent's `.git/config` file.
/// * `name` - The name of the submodule.
/// * `path` - The relative path of the submodule within the parent repository.
/// * `url` - The remote URL of the submodule.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err` if the config file cannot be opened or written to.
fn add_submodule_to_config(
    config_path: &Path,
    name: &str,
    path: &str,
    url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let existing_config = read_to_string(config_path).unwrap_or_default();

    let submodule_section = format!("[submodule \"{}\"]\n", name);
    if existing_config.contains(&submodule_section) {
        debug!("Submodule {} already in config", name);
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(config_path)?;

    writeln!(file, "[submodule \"{}\"]", name)?;
    writeln!(file, "\tactive = true")?;
    writeln!(file, "\turl = {}", url)?;
    writeln!(file, "\tpath = {}", path)?;

    debug!("Added submodule {} to .git/config", name);
    Ok(())
}

/// Fetches a single commit with `depth=1`, providing progress updates.
///
/// This is a high-level wrapper that first attempts to fetch the commit using the pure-Rust
/// `gix` library (`fetch_single_commit_gix`). If that fails, it falls back to using the
/// `git` command-line tool (`fetch_single_commit_git_cli`) for robustness.
///
/// # Arguments
///
/// * `dest_path` - The path to the local repository.
/// * `url` - The URL of the remote repository.
/// * `commit_sha` - The SHA of the commit to fetch.
/// * `tx` - An optional sender for sending `ProgressMessage` updates.
/// * `submodule_name` - An optional name for submodule-specific progress reporting.
///
/// # Returns
///
/// * `Ok(())` if the commit is fetched successfully by either method.
/// * `Err` if both `gix` and the `git` CLI fail.
pub fn fetch_single_commit(
    dest_path: &Path,
    url: &str,
    commit_sha: &str,
    tx: Option<Sender<ProgressMessage>>,
    submodule_name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    send_progress(&tx, submodule_name, 0);

    match fetch_single_commit_gix(dest_path, url, commit_sha, &tx, submodule_name) {
        Ok(()) => {
            debug!("Successfully fetched {} using gix", short_sha(commit_sha));
            send_progress(&tx, submodule_name, 100);
            Ok(())
        }
        Err(e) => {
            debug!(
                "gix fetch failed ({}), falling back to git CLI for {}",
                e,
                short_sha(commit_sha)
            );
            // Only fall back to CLI if git is available
            fetch_single_commit_git_cli(dest_path, url, commit_sha, &tx, submodule_name)
        }
    }
}

/// Fetches a single commit using the `gix` library with milestone-based progress.
///
/// This function performs a shallow fetch (`depth=1`) for a specific commit. It handles
/// both standard repositories and submodules (by resolving the `gitlink` file). If the
/// commit already exists locally, it skips the fetch and proceeds directly to checkout.
///
/// # Arguments
///
/// * `dest_path` - The path to the local repository or submodule worktree.
/// * `url` - The URL of the remote repository.
/// * `commit_sha` - The SHA of the commit to fetch.
/// * `tx` - An optional sender for sending `ProgressMessage` updates.
/// * `submodule_name` - An optional name for submodule-specific progress reporting.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err` if any stage of the `gix`-based fetch and checkout process fails.
fn fetch_single_commit_gix(
    dest_path: &Path,
    url: &str,
    commit_sha: &str,
    tx: &Option<Sender<ProgressMessage>>,
    submodule_name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let should_interrupt = &AtomicBool::new(false);

    // Parse the expected commit SHA upfront
    let expected_oid = parse_commit_oid(commit_sha)?;

    // Submodules have .git as a file (gitlink), not a directory
    let is_submodule = dest_path.join(".git").is_file();
    let actual_git_dir = resolve_actual_git_dir(dest_path, is_submodule)?;

    if actual_git_dir.exists()
        && checkout_if_commit_present(dest_path, expected_oid, commit_sha, tx, submodule_name)?
    {
        return Ok(());
    }

    let repo = open_or_init_fetch_repo(dest_path, &actual_git_dir, is_submodule)?;

    send_progress(tx, submodule_name, 10);

    // Parse the remote URL
    let remote_url = gix::url::parse(url).map_err(|e| format!("Invalid URL '{}': {}", url, e))?;

    send_progress(tx, submodule_name, 20);

    let outcome = fetch_commit_shallow_with_retry(&repo, &remote_url, commit_sha, should_interrupt)
        .map_err(|e| format!("fetch_single_commit_gix failed after 3 attempts: {}", e))?;

    send_progress(tx, submodule_name, 80);

    trace!(
        "Fetch complete: {} ref mappings",
        outcome.ref_map.mappings.len()
    );

    // Verify the commit exists
    let _commit = repo
        .find_commit(expected_oid)
        .map_err(|_| format!("Commit {} not found after fetch", short_sha(commit_sha)))?;

    // Checkout using gix
    checkout_commit_gix(&repo, expected_oid)?;

    Ok(())
}

/// The real git directory for `dest_path`: `<dest>/.git`, or for a submodule the
/// (canonicalized) target of the `gitdir:` line in its `.git` file.
fn resolve_actual_git_dir(
    dest_path: &Path,
    is_submodule: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let git_path = dest_path.join(".git");
    if !is_submodule {
        return Ok(git_path);
    }
    let gitlink_content = fs::read_to_string(&git_path)?;
    let git_dir_path = gitlink_content
        .trim()
        .strip_prefix("gitdir: ")
        .ok_or("Invalid gitlink file")?;
    Ok(dest_path.join(git_dir_path).canonicalize()?)
}

/// If the repository at `dest_path` already has `expected_oid`, checks it out and
/// returns `true`; returns `false` when the repo can't be opened or lacks the commit.
fn checkout_if_commit_present(
    dest_path: &Path,
    expected_oid: gix::ObjectId,
    commit_sha: &str,
    tx: &Option<Sender<ProgressMessage>>,
    submodule_name: Option<&str>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let Ok(repo) = gix::open(dest_path) else {
        return Ok(false);
    };
    if repo.find_commit(expected_oid).is_err() {
        return Ok(false);
    }
    debug!("Commit {} already exists locally", short_sha(commit_sha));
    send_progress(tx, submodule_name, 80);
    checkout_commit_gix(&repo, expected_oid)?;
    Ok(true)
}

fn open_or_init_fetch_repo(
    dest_path: &Path,
    actual_git_dir: &Path,
    is_submodule: bool,
) -> Result<gix::Repository, Box<dyn std::error::Error>> {
    if actual_git_dir.exists() {
        return Ok(gix::open(dest_path)?);
    }
    fs::create_dir_all(actual_git_dir)?;
    if !is_submodule {
        return Ok(gix::init(dest_path)?);
    }
    // For submodules, initialize in the modules directory
    gix::init(actual_git_dir)?;
    fs::write(actual_git_dir.join("HEAD"), "ref: refs/heads/master\n")?;
    Ok(gix::open(dest_path)?)
}

/// Checks out a specific commit's tree to the working directory using `gix`.
///
/// This function performs a basic checkout by iterating through the commit's tree and
/// writing each blob to its corresponding path in the working directory. It does not
/// update the Git index, but it does update the `HEAD` file to point to the given
/// commit, resulting in a detached HEAD state.
///
/// # Arguments
///
/// * `repo` - The `gix::Repository` to perform the checkout in.
/// * `commit_oid` - The `ObjectId` of the commit to check out.
///
/// # Returns
///
/// * `Ok(())` on successful checkout.
/// * `Err` if the repository has no workdir or if file I/O fails.
fn checkout_commit_gix(
    repo: &gix::Repository,
    commit_oid: gix::ObjectId,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Checking out commit {} to worktree", commit_oid);
    let commit = repo.find_commit(commit_oid)?;
    let tree = commit.tree()?;

    let worktree = repo
        .workdir()
        .ok_or("Repository has no working directory")?;

    // IMPORTANT: Remove everything except .git before checkout!
    for entry in fs::read_dir(worktree)? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name() != Some(std::ffi::OsStr::new(".git")) {
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
        }
    }

    // Now checkout the tree
    checkout_tree_recursive(repo, &tree, worktree)?;

    // Update the index
    populate_index_from_tree(repo, &tree)?;

    // Update HEAD
    let git_dir = repo.git_dir();
    fs::write(git_dir.join("HEAD"), format!("{}\n", commit_oid))?;

    Ok(())
}
/// Populate the index from a tree object
fn populate_index_from_tree(
    repo: &gix::Repository,
    tree: &gix::Tree,
) -> Result<(), Box<dyn std::error::Error>> {
    let workdir = repo
        .workdir()
        .ok_or("Repository has no working directory")?;

    // Collect all entries from the tree
    let mut entries = Vec::new();
    collect_tree_entries(tree, "", &mut entries, repo)?;

    // Populate real stat data so git status doesn't need to refresh the index
    for entry in &mut entries {
        let file_path = workdir.join(entry.path.to_str_lossy().as_ref());
        if let Ok(metadata) = fs::symlink_metadata(&file_path) {
            entry.stat = stat_from_metadata(&metadata);
        }
    }

    // Sort entries by path (required by git index format)
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    // Write a new index file directly
    let git_dir = repo.git_dir();
    let index_path = git_dir.join("index");

    // Get the repository's object hash kind (usually SHA1)
    let object_hash = repo.object_hash();

    // Create a new empty index with the correct hash
    let mut new_index =
        gix::index::File::from_state(gix::index::State::new(object_hash), index_path.clone());

    // Add all entries
    for entry in entries {
        new_index.dangerously_push_entry(
            entry.stat,
            entry.id,
            entry.flags,
            entry.mode.into(),
            entry.path.as_bstr(),
        );
    }

    // Write the new index
    let index_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&index_path)?;

    new_index
        .write_to(
            std::io::BufWriter::new(index_file),
            gix::index::write::Options::default(),
        )
        .map_err(gix::Error::from)?;

    Ok(())
}

/// Recursively collect all entries from a tree
fn collect_tree_entries(
    tree: &gix::Tree,
    prefix: &str,
    entries: &mut Vec<IndexEntryData>,
    repo: &gix::Repository,
) -> Result<(), Box<dyn std::error::Error>> {
    use gix::bstr::BString;

    for entry in tree.iter() {
        let entry = entry.map_err(gix::Error::from)?;
        let entry_mode = entry.mode();
        let entry_oid = entry.oid();
        let entry_name = entry.filename();

        // Build the full path
        let full_path = if prefix.is_empty() {
            entry_name.to_str_lossy().to_string()
        } else {
            format!("{}/{}", prefix, entry_name.to_str_lossy())
        };

        if entry_mode.is_tree() {
            // Recurse into subdirectory
            let subtree = repo.find_tree(entry_oid)?;
            collect_tree_entries(&subtree, &full_path, entries, repo)?;
        } else {
            // Add this entry (blob, executable, or gitlink/commit)
            entries.push(IndexEntryData {
                stat: gix::index::entry::Stat::default(),
                id: entry_oid.into(),
                flags: gix::index::entry::Flags::empty(),
                mode: entry_mode,
                path: BString::from(full_path),
            });
        }
    }

    Ok(())
}

/// Helper struct to collect index entry data
struct IndexEntryData {
    stat: gix::index::entry::Stat,
    id: gix::ObjectId,
    flags: gix::index::entry::Flags,
    mode: gix::objs::tree::EntryMode,
    path: gix::bstr::BString,
}

fn stat_from_metadata(metadata: &std::fs::Metadata) -> gix::index::entry::Stat {
    use std::time::UNIX_EPOCH;

    let mtime = metadata.modified().unwrap_or(UNIX_EPOCH);
    let mtime_dur = mtime.duration_since(UNIX_EPOCH).unwrap_or_default();

    let ctime = metadata
        .created()
        .or_else(|_| metadata.modified())
        .unwrap_or(UNIX_EPOCH);
    let ctime_dur = ctime.duration_since(UNIX_EPOCH).unwrap_or_default();

    let mut stat = gix::index::entry::Stat::default();
    stat.mtime.secs = mtime_dur.as_secs() as u32;
    stat.mtime.nsecs = mtime_dur.subsec_nanos();
    stat.ctime.secs = ctime_dur.as_secs() as u32;
    stat.ctime.nsecs = ctime_dur.subsec_nanos();
    stat.size = metadata.len() as u32;

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        stat.dev = metadata.dev() as u32;
        stat.ino = metadata.ino() as u32;
        stat.uid = metadata.uid();
        stat.gid = metadata.gid();
    }

    stat
}

/// Defines the type of Git reference to be checked out.
#[derive(Debug, Clone)]
pub enum GitReference {
    /// A branch reference.
    Branch(String),
    /// A tag reference.
    Tag(String),
    /// A specific commit hash.
    Commit(String),
    /// No specific reference; use the default from the clone.
    None,
}

/// Configuration options for cloning a Git repository.
#[derive(Debug)]
pub struct CloneOptions {
    /// The URL of the repository to clone.
    pub url: String,
    /// The local filesystem path where the repository will be cloned.
    pub path: String,
    /// The specific `GitReference` (branch, tag, or commit) to check out after cloning.
    pub reference: GitReference,
    /// If `true`, submodules will be initialized and updated recursively.
    pub recurse_submodules: bool,
    /// If `true`, a shallow clone (`depth=1`) will be performed.
    pub shallow: bool,
    /// Optional GitHub mirror prefix, equivalent to `git config url.<mirror>/.insteadOf https://github.com/`.
    pub mirror: Option<String>,
}

/// Checks out a specific `GitReference` (branch, tag, or commit) in a `gix` repository.
///
/// - For a `Branch`, it creates a local branch that tracks the remote branch and updates `HEAD` to point to it.
/// - For a `Tag` or `Commit`, it sets `HEAD` to a detached state pointing directly at the commit object.
/// - For `None`, it does nothing, leaving the repository at the default reference provided by the clone.
///
/// # Arguments
///
/// * `repo` - The `gix::Repository` to operate on.
/// * `reference` - The `GitReference` to check out.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err` if the reference cannot be found or the `HEAD` update fails.
fn checkout_reference(repo: &gix::Repository, reference: &GitReference) -> Result<()> {
    match reference {
        GitReference::Branch(branch) => checkout_branch(repo, branch),
        GitReference::Tag(tag) => {
            info!("Checking out tag: {}", tag);

            let refname = format!("refs/tags/{}", tag);
            let mut git_ref = repo.find_reference(&refname)?;

            let commit = git_ref.peel_to_commit()?;
            checkout_detached(repo, commit.id(), "tag", tag)
        }
        GitReference::Commit(commit_id) => {
            info!("Checking out commit: {}", commit_id);

            let oid = gix::ObjectId::from_hex(commit_id.as_bytes()).map_err(gix::Error::from)?;

            let commit = repo.find_commit(oid)?;
            checkout_detached(repo, commit.id(), "commit", commit_id)
        }
        GitReference::None => {
            debug!("Using default reference from clone");
            Ok(())
        }
    }
}

/// Creates a local branch from `origin/<branch>` (or the local branch / `HEAD` as fallbacks),
/// points `HEAD` at it and checks out its files.
fn checkout_branch(repo: &gix::Repository, branch: &str) -> Result<()> {
    info!("Checking out branch: {}", branch);

    let refname = format!("refs/remotes/origin/{}", branch);
    let mut git_ref = repo
        .find_reference(&refname)
        .or_else(|_| {
            debug!("Could not find remote ref {}, trying local branch", refname);
            repo.find_reference(&format!("refs/heads/{}", branch))
        })
        .or_else(|_| {
            debug!("Could not find local branch, trying packed refs");
            // For shallow clones, the branch might be in HEAD directly
            repo.find_reference("HEAD")
        })
        .map_err(|e| anyhow!("Could not find branch '{}': {}", branch, e))?;

    let commit = git_ref.peel_to_commit()?;

    // Create local branch
    let local_refname = format!("refs/heads/{}", branch);
    let name = gix::refs::FullName::try_from(local_refname.as_str())?;
    repo.reference(
        name,
        commit.id(),
        gix::refs::transaction::PreviousValue::Any,
        format!("branch: Created from origin/{}", branch),
    )?;

    set_head_to_ref(
        repo,
        &local_refname,
        &format!("checkout: moving to {}", branch),
    )?;
    checkout_files(repo, commit.id().into(), "branch", branch)
}

/// Detaches `HEAD` at `commit_id` and checks out its files.
fn checkout_detached(
    repo: &gix::Repository,
    commit_id: gix::Id,
    kind: &str,
    name: &str,
) -> Result<()> {
    set_head_detached(
        repo,
        commit_id,
        &format!("checkout: moving to {}", commit_id),
    )?;
    checkout_files(repo, commit_id.into(), kind, name)
}

fn checkout_files(
    repo: &gix::Repository,
    commit_oid: gix::ObjectId,
    kind: &str,
    name: &str,
) -> Result<()> {
    checkout_commit_gix(repo, commit_oid).map_err(|e| {
        error!("Error checking out files for {} {}: {}", kind, name, e);
        anyhow::anyhow!("{}", e)
    })
}

/// Sets the repository's `HEAD` to a detached state pointing at a specific commit ID.
///
/// # Arguments
///
/// * `repo` - The `gix::Repository` to modify.
/// * `commit_id` - The `gix::Id` of the commit to detach `HEAD` at.
/// * `message` - The reflog message for this change.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err` if the reference edit fails.
fn set_head_detached(repo: &gix::Repository, commit_id: gix::Id, message: &str) -> Result<()> {
    set_head(repo, Target::Object(commit_id.detach()), message)
}

fn set_head(repo: &gix::Repository, new: Target, message: &str) -> Result<()> {
    let edit = RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: message.into(),
            },
            expected: PreviousValue::Any,
            new,
        },
        name: gix::refs::FullName::try_from("HEAD")?,
        deref: false,
    };

    repo.edit_reference(edit)?;
    Ok(())
}

/// Sets the repository's `HEAD` to be a symbolic reference pointing to another reference (e.g., a branch).
///
/// # Arguments
///
/// * `repo` - The `gix::Repository` to modify.
/// * `refname` - The full name of the reference that `HEAD` should point to (e.g., "refs/heads/master").
/// * `message` - The reflog message for this change.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err` if the reference edit fails.
fn set_head_to_ref(repo: &gix::Repository, refname: &str, message: &str) -> Result<()> {
    set_head(
        repo,
        Target::Symbolic(gix::refs::FullName::try_from(refname)?),
        message,
    )
}

/// Clones the ESP-IDF repository with specified options.
///
/// Tries the pure-Rust `gix`-based [`clone_repository`] first. If that fails
/// **and** the destination directory was either absent or empty before this
/// call started, the function falls back to the system `git` binary via
/// [`clone_with_git_cli`]. Both paths honor the same `shallow` and
/// `recurse_submodules` semantics, including shallow (depth=1) submodule
/// clones.
///
/// # Data safety
///
/// The fallback path will **never** delete pre-existing user content. If
/// `path` already contained files (for example, a previous clone, a `.git`
/// directory, or unrelated user data) before this function was called, a
/// failure of the gix clone is returned as-is and the destination is left
/// untouched. Only contents that this call itself produced (in a previously
/// empty or non-existent directory) are removed before retrying.
///
/// # Arguments
///
/// * `path` - Local path where the repository should be cloned.
/// * `repository` - Optional `owner/name` repo string.
/// * `version` - Branch (`master`, `release-xxx`), tag (e.g. `v5.1.2`), or 40-char commit SHA.
/// * `mirror` - Optional mirror URL prefix (e.g. Gitee).
/// * `with_submodules` - If `true`, recursively initialize submodules.
/// * `tx` - Progress channel.
///
/// # Returns
///
/// * `Ok(String)` with the cloned path on success (from either path).
/// * `Err(String)` if the gix clone fails and either the fallback also fails
///   or the fallback was skipped because pre-existing content was detected.
pub fn get_esp_idf(
    path: &str,
    repository: Option<&str>,
    version: &str,
    mirror: Option<&str>,
    with_submodules: bool,
    tx: Sender<ProgressMessage>,
) -> Result<String, String> {
    let dest_path = std::path::PathBuf::from(path);

    let had_preexisting_content = path_has_content(&dest_path);

    // Ensure the path exists (may create an empty directory).
    let _ = ensure_path(path);

    let url = get_repo_url(repository, mirror);

    let shallow = true;
    let reference = git_reference_from_version(version);

    let mirror_owned = mirror.map(|s| s.to_string());
    let clone_options = CloneOptions {
        url,
        path: path.to_string(),
        reference,
        recurse_submodules: with_submodules,
        shallow,
        mirror: mirror_owned,
    };

    // First attempt: pure-Rust gix-based clone. This is faster and more efficient and result is smaller, but may fail on some platforms or network conditions.
    //
    // `clone_repository` consumes its `CloneOptions`, so we hand it a freshly
    // rebuilt copy and keep the original around for the potential fallback.
    let gix_attempt = clone_repository(
        CloneOptions {
            url: clone_options.url.clone(),
            path: clone_options.path.clone(),
            reference: clone_options.reference.clone(),
            recurse_submodules: clone_options.recurse_submodules,
            shallow: clone_options.shallow,
            mirror: clone_options.mirror.clone(),
        },
        tx.clone(),
    );

    match gix_attempt {
        Ok(repo) => Ok(repo.to_str().unwrap_or(path).to_string()),
        Err(gix_err) => {
            if had_preexisting_content {
                let _ = tx.send(ProgressMessage::Finish);
                error!(
                    "gix-based clone failed and {} already contained data \
              before this call; refusing to wipe it. Move or remove \
              the directory manually and retry.",
                    dest_path.display()
                );
                return Err(format!(
                    "gix clone failed and destination {} is not empty; \
              refusing to delete pre-existing content. Original error: {}",
                    dest_path.display(),
                    gix_err
                ));
            }

            warn!(
                "gix-based clone of ESP-IDF failed ({}); falling back to system git",
                gix_err
            );

            if dest_path.exists() {
                if let Err(e) = std::fs::remove_dir_all(&dest_path) {
                    debug!(
                        "Failed to fully clean {} before fallback clone: {}",
                        dest_path.display(),
                        e
                    );
                }
            }

            match clone_with_git_cli(&clone_options, tx.clone()) {
                Ok(repo) => {
                    let _ = tx.send(ProgressMessage::Finish);
                    Ok(repo.to_str().unwrap_or(path).to_string())
                }
                Err(cli_err) => {
                    let _ = tx.send(ProgressMessage::Finish);
                    Err(format!(
                        "Both gix and system git failed to clone ESP-IDF. \
                gix error: {gix_err}; git CLI error: {cli_err}"
                    ))
                }
            }
        }
    }
}

/// `true` if `path` exists and is either a non-directory or a non-empty directory.
fn path_has_content(path: &Path) -> bool {
    path.exists()
        && (!path.is_dir()
            || std::fs::read_dir(path)
                .map(|mut it| it.next().is_some())
                .unwrap_or(false))
}

/// Maps an ESP-IDF version string to a reference: `master` and `release-*` are branches
/// (`release-vX.Y` becomes `release/vX.Y`), a 40-char hex string is a commit, anything else a tag.
fn git_reference_from_version(version: &str) -> GitReference {
    if version == "master" {
        GitReference::Branch("master".to_string())
    } else if version.contains("release") {
        GitReference::Branch(version.to_string().replace("release-", "release/"))
    } else if version.len() == 40 && version.chars().all(|c| c.is_ascii_hexdigit()) {
        GitReference::Commit(version.to_string())
    } else {
        GitReference::Tag(version.to_string())
    }
}

/// Rewrites a mirror URL back to the equivalent `https://github.com/` URL.
///
/// Relative submodule paths in `.gitmodules` are defined against the GitHub
/// layout; resolving them against a mirror prefix (e.g. `/esp-mirror/`) yields
/// incorrect URLs. This function restores the logical GitHub parent first.
pub fn reverse_github_mirror(url: &str, mirror: Option<&str>) -> String {
    let Some(mirror) = mirror else {
        return url.to_string();
    };
    if mirror == "https://github.com" {
        return url.to_string();
    }
    let mirror_prefix = format!("{}/", mirror.trim_end_matches('/'));
    if url.starts_with(&mirror_prefix) {
        url.replacen(&mirror_prefix, "https://github.com/", 1)
    } else {
        url.to_string()
    }
}

/// Rewrites `https://github.com/` URLs to use the configured mirror prefix.
///
/// Equivalent to `git config url.<mirror>/.insteadOf https://github.com/`.
pub fn apply_github_mirror(url: &str, mirror: Option<&str>) -> String {
    let Some(mirror) = mirror else {
        return url.to_string();
    };
    if mirror == "https://github.com" {
        return url.to_string();
    }
    let mirror_base = mirror.trim_end_matches('/');
    let mirror_prefix = format!("{}/", mirror_base);
    url.replace("https://github.com/", &mirror_prefix)
        .replace("https://github.com", mirror_base)
}

fn git_instead_of_config_pair(mirror: Option<&str>) -> Option<(String, String)> {
    let mirror = mirror?;
    if mirror == "https://github.com" {
        return None;
    }
    let mirror_prefix = format!("{}/", mirror.trim_end_matches('/'));
    Some((
        format!("url.{}.insteadOf", mirror_prefix),
        "https://github.com/".to_string(),
    ))
}

fn configure_local_github_mirror(
    repo_path: &str,
    mirror: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some((key, value)) = git_instead_of_config_pair(mirror) {
        run_git_checked(
            &["config", &key, &value],
            repo_path,
            &format!("git config {}", key),
        )?;
    }
    Ok(())
}

/// Constructs the full git repository URL from optional repository and mirror parts.
///
/// It intelligently combines the base URL from the mirror (or GitHub by default) with
/// the repository path.
///
/// # Arguments
///
/// * `repository` - An optional repository string (e.g., "espressif/esp-idf"). Defaults to a suitable value for ESP-IDF.
/// * `mirror` - An optional mirror URL prefix (e.g., "https://gitee.com").
///
/// # Returns
///
/// A `String` containing the full URL for the repository.
pub fn get_repo_url(repository: Option<&str>, mirror: Option<&str>) -> String {
    // Determine the repository URL
    let repo_part_url = match repository {
        Some(repo) => format!("{}.git", repo),
        None => {
            if mirror.is_some_and(|m| m.contains("https://gitee.com/")) {
                "EspressifSystems/esp-idf.git".to_string()
            } else {
                "espressif/esp-idf.git".to_string()
            }
        }
    };

    let url = match mirror {
        Some(url) => format!("{}/{}", url, repo_part_url),
        None => format!("https://github.com/{}", repo_part_url),
    };
    url
}

/// Constructs the URL for fetching a single raw file from a git repository host.
///
/// This function builds the correct URL format for accessing a raw file based on the
/// hosting platform (e.g., GitHub, Gitee, GitLab). It handles different URL structures
/// and reference naming conventions.
///
/// # Arguments
///
/// * `repository` - An optional repository string (e.g., "espressif/esp-idf").
/// * `version` - The git reference (branch, tag, or commit) containing the file.
/// * `mirror` - An optional mirror URL, used to detect the hosting platform.
/// * `file_path` - The path to the file within the repository.
///
/// # Returns
///
/// A `String` containing the full URL to the raw file.
pub fn get_raw_file_url(
    repository: Option<&str>,
    version: &str,
    mirror: Option<&str>,
    file_path: &str,
) -> String {
    // Determine the repository name
    let repo_name = match repository {
        Some(repo) => repo.to_string(),
        None => {
            if mirror.is_some_and(|m| m.contains("https://gitee.com/")) {
                "EspressifSystems/esp-idf".to_string()
            } else {
                "espressif/esp-idf".to_string()
            }
        }
    };

    // Normalize the version/reference
    let ref_name = if version == "master" {
        "master".to_string()
    } else if version.contains("release") {
        version.replace("release-", "release/")
    } else if version.len() == 40 && version.chars().all(|c| c.is_ascii_hexdigit()) {
        // Commit hash
        version.to_string()
    } else {
        // Tag - need to prepend 'v' if not present for esp-idf tags
        if version.starts_with('v') {
            version.to_string()
        } else {
            format!("v{}", version)
        }
    };

    // Build the raw file URL based on the hosting platform
    if let Some(mirror_url) = mirror {
        if mirror_url.contains("gitee.com") {
            // Gitee raw format: https://gitee.com/owner/repo/raw/branch/path
            format!(
                "{}/{}/raw/{}/{}",
                mirror_url, repo_name, ref_name, file_path
            )
        } else if mirror_url.contains("gitlab") {
            // GitLab raw format: https://gitlab.com/owner/repo/-/raw/branch/path
            format!(
                "{}/{}/-/raw/{}/{}",
                mirror_url, repo_name, ref_name, file_path
            )
        } else {
            // Generic git hosting - try GitHub format
            format!(
                "{}/{}/raw/{}/{}",
                mirror_url, repo_name, ref_name, file_path
            )
        }
    } else {
        // Default to GitHub raw format: https://raw.githubusercontent.com/owner/repo/branch/path
        format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            repo_name, ref_name, file_path
        )
    }
}

/// Performs a full repository clone using the system `git` command-line tool.
///
/// This function mirrors the behavior of [`clone_repository`] but uses the
/// system `git` binary instead of `gix`. It is intended as a fallback for
/// environments where the pure-Rust implementation cannot complete the clone
/// (e.g. due to protocol/CDN quirks or unsupported features).
///
/// Behavior:
/// * If `options.shallow` is `true`, the clone is performed with `--depth 1`.
///   For branches and tags, `--branch <name> --single-branch` is also passed.
///   For raw commit SHAs, the default branch is shallow-cloned first and then
///   `git fetch --depth 1 origin <sha>` is attempted, falling back to a
///   non-shallow fetch if the server rejects fetching by SHA.
/// * If `options.recurse_submodules` is `true`, submodules are initialized
///   with `git submodule update --init --recursive --depth 1` so each
///   submodule is also a shallow clone of a single commit.
/// * Progress is parsed from `git`'s stderr (via `--progress`) and forwarded
///   through `tx` as [`ProgressMessage::Update`] for the main clone and
///   [`ProgressMessage::SubmoduleUpdate`] for submodules. Note that progress
///   streaming only takes effect when `spawn_with_dir` pipes stderr; if it
///   inherits stderr the milestone updates (0 / 80 / 100) still fire.
///
/// All command invocations go through [`crate::command_executor`] so that
/// platform-specific concerns (e.g. hidden console windows on Windows) are
/// handled consistently.
///
/// # Arguments
///
/// * `options` - Clone configuration (URL, destination, reference, flags).
/// * `tx`      - Sender used to report progress to the caller.
///
/// # Returns
///
/// * `Ok(PathBuf)` - The destination path on success.
/// * `Err`         - If any underlying `git` command fails.
pub fn clone_with_git_cli(
    options: &CloneOptions,
    tx: Sender<ProgressMessage>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dest_path = PathBuf::from(&options.path);

    info!(
        "Cloning {} into {} via system git (fallback)",
        options.url,
        dest_path.display()
    );

    if let Some(parent) = dest_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    if path_has_content(&dest_path) {
        return Err(format!(
            "Destination {} exists and is not empty; refusing to clone over it",
            dest_path.display()
        )
        .into());
    }

    let cwd = match dest_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::env::current_dir()?,
    };
    let cwd_str = cwd
        .to_str()
        .ok_or("Working directory contains non-UTF-8 characters")?;

    let dest_str = dest_path
        .to_str()
        .ok_or("Destination path contains non-UTF-8 characters")?;

    let plan = git_clone_plan(options, dest_str);

    // 0% - starting
    let _ = tx.send(ProgressMessage::Update(0));

    run_git_clone(&plan.args, cwd_str, &tx, &options.url)?;

    let _ = tx.send(ProgressMessage::Update(80));

    configure_local_github_mirror(dest_str, options.mirror.as_deref())?;

    // If the user asked for a specific commit, fetch + checkout it now.
    if let Some(sha) = plan.post_checkout_commit.as_deref() {
        info!("Fetching specific commit {} via git CLI fallback", sha);
        fetch_commit_for_cli_checkout(dest_str, sha)?;
        checkout_with_git_cli(&dest_path, sha)?;
    }

    // Run submodule update separately when we couldn't recurse during clone.
    if options.recurse_submodules && !plan.recurse_during_clone {
        info!("Initializing submodules via git CLI fallback (shallow)");
        run_git_submodule_update(&submodule_update_args(options.shallow), dest_str, &tx)?;
    }

    let _ = tx.send(ProgressMessage::Update(100));
    Ok(dest_path)
}

/// `git clone` invocation for [`clone_with_git_cli`] plus the follow-up work it implies.
#[derive(Debug, PartialEq)]
struct GitClonePlan {
    args: Vec<String>,
    /// Commit to fetch and check out after cloning (a clone can't target a raw SHA).
    post_checkout_commit: Option<String>,
    /// Whether submodules are cloned by `git clone --recurse-submodules` itself.
    recurse_during_clone: bool,
}

fn git_clone_plan(options: &CloneOptions, dest_str: &str) -> GitClonePlan {
    let clone_url = reverse_github_mirror(&options.url, options.mirror.as_deref());
    let mut args: Vec<String> = vec!["clone".to_string(), "--progress".to_string()];
    if let Some((key, value)) = git_instead_of_config_pair(options.mirror.as_deref()) {
        args.push("-c".to_string());
        args.push(format!("{}={}", key, value));
    }

    if options.shallow {
        args.push("--depth".to_string());
        args.push("1".to_string());
    }

    let post_checkout_commit = match &options.reference {
        GitReference::Branch(name) | GitReference::Tag(name) => {
            args.push("--branch".to_string());
            args.push(name.clone());
            args.push("--single-branch".to_string());
            None
        }
        GitReference::Commit(sha) => Some(sha.clone()),
        GitReference::None => None,
    };

    // When a post-clone commit checkout is needed, submodules are updated separately so
    // their SHAs match the checked-out commit instead of the default branch tip.
    let recurse_during_clone = options.recurse_submodules && post_checkout_commit.is_none();
    if recurse_during_clone {
        args.push("--recurse-submodules".to_string());
        if options.shallow {
            args.push("--shallow-submodules".to_string());
        }
    }

    args.push(clone_url);
    args.push(dest_str.to_string());

    GitClonePlan {
        args,
        post_checkout_commit,
        recurse_during_clone,
    }
}

const STDERR_TAIL_LINES: usize = 20;

/// Spawns `git clone`, forwarding its progress scaled into 0-80% (leaving headroom for
/// post-clone work) and reporting the tail of git's diagnostics on failure.
fn run_git_clone(
    args: &[String],
    cwd: &str,
    tx: &Sender<ProgressMessage>,
    url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let mut child = spawn_with_dir("git", &args_ref, cwd)?;
    let mut stderr_tail: Vec<String> = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            match parse_git_progress(&line) {
                Some(percentage) => {
                    let _ = tx.send(ProgressMessage::Update((percentage * 80) / 100));
                }
                None => push_stderr_tail(&mut stderr_tail, &line),
            }
        }
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(git_clone_failure_message(status.code(), url, &stderr_tail).into());
    }
    Ok(())
}

/// Keeps the last [`STDERR_TAIL_LINES`] non-empty, trimmed stderr lines.
fn push_stderr_tail(tail: &mut Vec<String>, line: &str) {
    let trimmed = line.trim();
    if !trimmed.is_empty() {
        tail.push(trimmed.to_string());
        if tail.len() > STDERR_TAIL_LINES {
            tail.remove(0);
        }
    }
}

fn git_clone_failure_message(code: Option<i32>, url: &str, stderr_tail: &[String]) -> String {
    let details = stderr_tail.join("; ");
    format!(
        "git clone failed (exit code {:?}) for {}{}",
        code,
        url,
        if details.is_empty() {
            String::new()
        } else {
            format!(": {}", details)
        }
    )
}

/// Makes `sha` available locally: a depth-1 fetch by SHA first, then `--unshallow`,
/// then a plain fetch.
fn fetch_commit_for_cli_checkout(
    dest_str: &str,
    sha: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Many servers support fetching by SHA when `uploadpack.allowReachableSHA1InWant`
    // is enabled (GitHub does).
    let shallow_fetch =
        execute_command_with_dir("git", &["fetch", "--depth", "1", "origin", sha], dest_str)?;
    if shallow_fetch.status.success() {
        return Ok(());
    }
    debug!(
        "Shallow fetch of commit {} failed, retrying without --depth: {}",
        sha,
        String::from_utf8_lossy(&shallow_fetch.stderr)
    );

    // Unshallowing errors out if the repo is already complete; then do a plain fetch.
    let unshallow = execute_command_with_dir("git", &["fetch", "--unshallow", "origin"], dest_str)?;
    if unshallow.status.success() {
        return Ok(());
    }
    let plain_fetch = execute_command_with_dir("git", &["fetch", "origin"], dest_str)?;
    if !plain_fetch.status.success() {
        return Err(format!(
            "git fetch failed while resolving commit {}: {}",
            sha,
            String::from_utf8_lossy(&plain_fetch.stderr)
        )
        .into());
    }
    Ok(())
}

fn submodule_update_args(shallow: bool) -> Vec<&'static str> {
    let mut args = vec!["submodule", "update", "--init", "--recursive", "--progress"];
    if shallow {
        args.push("--depth");
        args.push("1");
    }
    args
}

/// Runs `git submodule update`, streaming per-submodule progress from its stderr.
fn run_git_submodule_update(
    args: &[&str],
    dest_str: &str,
    tx: &Sender<ProgressMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut child = spawn_with_dir("git", args, dest_str)?;
    let mut tracker = SubmoduleProgressTracker::default();
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            tracker.handle_line(&line, tx);
        }
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(format!(
            "git submodule update failed (exit code {:?})",
            status.code()
        )
        .into());
    }
    tracker.finish(tx);
    Ok(())
}

/// Attributes `git submodule update` progress lines to the submodule currently being cloned.
#[derive(Default)]
struct SubmoduleProgressTracker {
    current: Option<String>,
}

impl SubmoduleProgressTracker {
    fn handle_line(&mut self, line: &str, tx: &Sender<ProgressMessage>) {
        if let Some(name) = parse_submodule_name(line) {
            if let Some(prev) = self.current.replace(name.clone()) {
                if prev != name {
                    let _ = tx.send(ProgressMessage::SubmoduleFinish(prev));
                }
            }
        }
        if let (Some(percentage), Some(name)) = (parse_git_progress(line), self.current.as_deref())
        {
            let _ = tx.send(ProgressMessage::SubmoduleUpdate((
                name.to_string(),
                percentage,
            )));
        }
    }

    fn finish(&mut self, tx: &Sender<ProgressMessage>) {
        if let Some(name) = self.current.take() {
            let _ = tx.send(ProgressMessage::SubmoduleFinish(name));
        }
    }
}

/// Best-effort parse of `git submodule` stderr to figure out which submodule
/// a progress line belongs to.
///
/// Recognizes lines like:
/// * `Submodule path 'components/foo': ...`
/// * `Cloning into '/abs/path/components/foo'...`
fn parse_submodule_name(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("Submodule path '") {
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }
    if let Some(rest) = line.strip_prefix("Cloning into '") {
        if let Some(end) = rest.rfind('\'') {
            let path = &rest[..end];
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(path);
            return Some(name.to_string());
        }
    }
    None
}

#[cfg(test)]
mod mirror_url_tests {
    use super::*;

    const JIHULAB_MIRROR: &str = "https://jihulab.com/esp-mirror";

    #[test]
    fn apply_github_mirror_rewrites_absolute_urls() {
        let url = "https://github.com/espressif/esp32-bt-lib.git";
        assert_eq!(
            apply_github_mirror(url, Some(JIHULAB_MIRROR)),
            "https://jihulab.com/esp-mirror/espressif/esp32-bt-lib.git"
        );
    }

    #[test]
    fn reverse_github_mirror_restores_github_prefix() {
        let url = "https://jihulab.com/esp-mirror/espressif/esp-idf.git";
        assert_eq!(
            reverse_github_mirror(url, Some(JIHULAB_MIRROR)),
            "https://github.com/espressif/esp-idf.git"
        );
    }

    #[test]
    fn relative_submodule_url_uses_github_base_then_mirror() {
        let parent = "https://jihulab.com/esp-mirror/espressif/esp-idf.git";
        let resolution_base = reverse_github_mirror(parent, Some(JIHULAB_MIRROR));
        let resolved =
            resolve_submodule_url("../../espressif/esp32-bt-lib.git", &resolution_base).unwrap();
        let mirrored = apply_github_mirror(&resolved, Some(JIHULAB_MIRROR));
        assert_eq!(
            mirrored,
            "https://jihulab.com/esp-mirror/espressif/esp32-bt-lib.git"
        );
    }

    #[test]
    fn relative_submodule_url_without_mirror_fix_is_wrong() {
        let parent = "https://jihulab.com/esp-mirror/espressif/esp-idf.git";
        let wrong = resolve_submodule_url("../../espressif/esp32-bt-lib.git", parent).unwrap();
        assert_eq!(wrong, "https://jihulab.com/espressif/esp32-bt-lib.git");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, Receiver};
    use tempfile::TempDir;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    fn drain(rx: &Receiver<ProgressMessage>) -> Vec<String> {
        rx.try_iter()
            .map(|msg| match msg {
                ProgressMessage::Update(p) => format!("update:{p}"),
                ProgressMessage::Finish => "finish".to_string(),
                ProgressMessage::SubmoduleUpdate((name, p)) => format!("sub:{name}:{p}"),
                ProgressMessage::SubmoduleFinish(name) => format!("sub-finish:{name}"),
            })
            .collect()
    }

    fn options(reference: GitReference, shallow: bool, recurse: bool) -> CloneOptions {
        CloneOptions {
            url: "https://github.com/espressif/esp-idf.git".to_string(),
            path: "/tmp/esp-idf".to_string(),
            reference,
            recurse_submodules: recurse,
            shallow,
            mirror: None,
        }
    }

    #[test]
    fn scale_fetch_progress_starts_at_ten_percent() {
        assert_eq!(scale_fetch_progress(0), 10);
        assert_eq!(scale_fetch_progress(3), 12);
    }

    #[test]
    fn clone_shallow_mode_is_depth_one_except_for_commits() {
        use gix::remote::fetch::Shallow;
        let branch = GitReference::Branch("master".into());
        let commit = GitReference::Commit(SHA.into());
        assert!(matches!(
            clone_shallow_mode(true, &branch),
            Shallow::DepthAtRemote(d) if d.get() == 1
        ));
        assert!(matches!(
            clone_shallow_mode(true, &GitReference::None),
            Shallow::DepthAtRemote(_)
        ));
        assert!(matches!(
            clone_shallow_mode(true, &commit),
            Shallow::NoChange
        ));
        assert!(matches!(
            clone_shallow_mode(false, &branch),
            Shallow::NoChange
        ));
    }

    #[test]
    fn clone_fetch_refspec_targets_branch_or_tag_only() {
        assert_eq!(
            clone_fetch_refspec(&GitReference::Branch("release/v5.1".into())).as_deref(),
            Some("+refs/heads/release/v5.1:refs/remotes/origin/release/v5.1")
        );
        assert_eq!(
            clone_fetch_refspec(&GitReference::Tag("v5.1.2".into())).as_deref(),
            Some("+refs/tags/v5.1.2:refs/tags/v5.1.2")
        );
        assert_eq!(clone_fetch_refspec(&GitReference::Commit(SHA.into())), None);
        assert_eq!(clone_fetch_refspec(&GitReference::None), None);
    }

    #[test]
    fn insert_after_core_section_needs_core_header_line() {
        let mut config = "[core]\n\tbare = false\n".to_string();
        assert!(insert_after_core_section(&mut config, "\tx = 1\n"));
        assert_eq!(config, "[core]\n\tx = 1\n\tbare = false\n");

        let mut no_core = "[remote \"origin\"]\n".to_string();
        assert!(!insert_after_core_section(&mut no_core, "\tx = 1\n"));
        assert_eq!(no_core, "[remote \"origin\"]\n");

        let mut no_newline = "[core]".to_string();
        assert!(!insert_after_core_section(&mut no_newline, "\tx = 1\n"));
    }

    #[test]
    fn config_with_symlinks_disabled_cases() {
        assert_eq!(
            config_with_symlinks_disabled("[core]\n\tsymlinks = true\n").as_deref(),
            Some("[core]\n\tsymlinks = false\n")
        );
        assert_eq!(
            config_with_symlinks_disabled("[core]\n\tsymlinks = false\n"),
            None
        );
        assert_eq!(
            config_with_symlinks_disabled("[core]\n\tbare = false\n").as_deref(),
            Some("[core]\n\tsymlinks = false\n\tbare = false\n")
        );
        assert_eq!(config_with_symlinks_disabled("[user]\n"), None);
    }

    #[test]
    fn submodule_repo_config_makes_bare_repo_a_submodule() {
        let bare = "[core]\n\trepositoryformatversion = 0\n\tbare = true\n";
        let config = submodule_repo_config(
            bare.to_string(),
            Path::new("../../../components/foo"),
            "https://github.com/espressif/foo.git",
        );
        assert!(config.contains("bare = false"));
        assert!(!config.contains("bare = true"));
        assert!(config.starts_with("[core]\n\tworktree = ../../../components/foo\n"));
        assert!(config.ends_with(
            "\n[remote \"origin\"]\n\turl = https://github.com/espressif/foo.git\n\
             \tfetch = +refs/heads/*:refs/remotes/origin/*\n"
        ));
    }

    #[test]
    fn submodule_repo_config_keeps_existing_worktree_and_remote() {
        let existing = "[core]\n\tworktree = ../x\n[remote \"origin\"]\n\turl = u\n";
        let config = submodule_repo_config(existing.to_string(), Path::new("../y"), "other");
        #[cfg(not(windows))]
        assert_eq!(config, existing);
        assert!(!config.contains("../y"));
        assert!(!config.contains("other"));
    }

    #[test]
    fn resolve_relative_segments_drops_repo_name_then_applies_relative() {
        assert_eq!(
            resolve_relative_segments(vec!["espressif", "esp-idf.git"], "../other/lib.git"),
            vec!["other", "lib.git"]
        );
        assert_eq!(
            resolve_relative_segments(vec!["a", "b", "repo"], "./c//d"),
            vec!["a", "b", "c", "d"]
        );
        assert_eq!(
            resolve_relative_segments(vec!["repo"], "../../x"),
            vec!["x"]
        );
        assert!(resolve_relative_segments(vec![], "..").is_empty());
    }

    #[test]
    fn resolve_relative_urls_for_http_and_ssh_parents() {
        assert_eq!(
            resolve_http_relative_url("../lib.git", "https://github.com/espressif/esp-idf.git")
                .unwrap(),
            "https://github.com/lib.git"
        );
        assert_eq!(
            resolve_ssh_relative_url("../lib.git", "git@github.com:espressif/esp-idf.git").unwrap(),
            "git@github.com:lib.git"
        );
        assert_eq!(
            resolve_ssh_relative_url("./lib.git", "git@github.com:espressif/esp-idf.git").unwrap(),
            "git@github.com:espressif/lib.git"
        );
        assert!(resolve_ssh_relative_url("../x", "ssh://a:b:c").is_err());
    }

    #[test]
    fn short_sha_truncates_to_seven_chars() {
        assert_eq!(short_sha(SHA), "0123456");
        assert_eq!(short_sha("abc"), "abc");
    }

    #[test]
    fn parse_commit_oid_accepts_full_hex_and_reports_bad_input() {
        assert_eq!(parse_commit_oid(SHA).unwrap().to_string(), SHA);
        let err = parse_commit_oid("not-a-sha").unwrap_err().to_string();
        assert!(err.starts_with("Invalid SHA 'not-a-sha': "), "{err}");
    }

    #[test]
    fn path_has_content_cases() {
        let tmp = TempDir::new().unwrap();
        assert!(!path_has_content(&tmp.path().join("missing")));
        assert!(!path_has_content(tmp.path()));
        let file = tmp.path().join("file.txt");
        fs::write(&file, "x").unwrap();
        assert!(path_has_content(tmp.path()));
        assert!(path_has_content(&file));
    }

    #[test]
    fn git_reference_from_version_cases() {
        assert!(matches!(
            git_reference_from_version("master"),
            GitReference::Branch(b) if b == "master"
        ));
        assert!(matches!(
            git_reference_from_version("release-v5.1"),
            GitReference::Branch(b) if b == "release/v5.1"
        ));
        assert!(matches!(
            git_reference_from_version(SHA),
            GitReference::Commit(c) if c == SHA
        ));
        assert!(matches!(
            git_reference_from_version("v5.1.2"),
            GitReference::Tag(t) if t == "v5.1.2"
        ));
        assert!(matches!(
            git_reference_from_version(&SHA[..39]),
            GitReference::Tag(_)
        ));
    }

    #[test]
    fn git_clone_plan_for_shallow_branch_with_submodules() {
        let plan = git_clone_plan(
            &options(GitReference::Branch("master".into()), true, true),
            "/tmp/esp-idf",
        );
        assert_eq!(
            plan,
            GitClonePlan {
                args: [
                    "clone",
                    "--progress",
                    "--depth",
                    "1",
                    "--branch",
                    "master",
                    "--single-branch",
                    "--recurse-submodules",
                    "--shallow-submodules",
                    "https://github.com/espressif/esp-idf.git",
                    "/tmp/esp-idf",
                ]
                .map(String::from)
                .to_vec(),
                post_checkout_commit: None,
                recurse_during_clone: true,
            }
        );
    }

    #[test]
    fn git_clone_plan_for_commit_defers_checkout_and_submodules() {
        let plan = git_clone_plan(
            &options(GitReference::Commit(SHA.into()), false, true),
            "dest",
        );
        assert_eq!(
            plan.args,
            [
                "clone",
                "--progress",
                "https://github.com/espressif/esp-idf.git",
                "dest"
            ]
        );
        assert_eq!(plan.post_checkout_commit.as_deref(), Some(SHA));
        assert!(!plan.recurse_during_clone);
    }

    #[test]
    fn git_clone_plan_with_mirror_clones_github_url_through_insteadof() {
        let mut opts = options(GitReference::Tag("v5.1.2".into()), false, false);
        opts.url = "https://jihulab.com/esp-mirror/espressif/esp-idf.git".to_string();
        opts.mirror = Some("https://jihulab.com/esp-mirror".to_string());
        let plan = git_clone_plan(&opts, "dest");
        assert_eq!(
            plan.args,
            [
                "clone",
                "--progress",
                "-c",
                "url.https://jihulab.com/esp-mirror/.insteadOf=https://github.com/",
                "--branch",
                "v5.1.2",
                "--single-branch",
                "https://github.com/espressif/esp-idf.git",
                "dest"
            ]
        );
        assert!(!plan.recurse_during_clone);
    }

    #[test]
    fn git_clone_plan_without_reference_or_recursion() {
        let plan = git_clone_plan(&options(GitReference::None, false, false), "dest");
        assert_eq!(plan.args.len(), 4);
        assert_eq!(plan.post_checkout_commit, None);
        assert!(!plan.recurse_during_clone);
    }

    #[test]
    fn push_stderr_tail_trims_skips_blank_and_is_bounded() {
        let mut tail = Vec::new();
        push_stderr_tail(&mut tail, "   ");
        assert!(tail.is_empty());
        for i in 0..25 {
            push_stderr_tail(&mut tail, &format!("  line {i}  "));
        }
        assert_eq!(tail.len(), STDERR_TAIL_LINES);
        assert_eq!(tail.first().map(String::as_str), Some("line 5"));
        assert_eq!(tail.last().map(String::as_str), Some("line 24"));
    }

    #[test]
    fn git_clone_failure_message_with_and_without_details() {
        assert_eq!(
            git_clone_failure_message(Some(128), "u", &[]),
            "git clone failed (exit code Some(128)) for u"
        );
        assert_eq!(
            git_clone_failure_message(None, "u", &["fatal: a".into(), "fatal: b".into()]),
            "git clone failed (exit code None) for u: fatal: a; fatal: b"
        );
    }

    #[test]
    fn submodule_update_args_add_depth_when_shallow() {
        assert_eq!(
            submodule_update_args(false),
            ["submodule", "update", "--init", "--recursive", "--progress"]
        );
        assert_eq!(
            submodule_update_args(true),
            [
                "submodule",
                "update",
                "--init",
                "--recursive",
                "--progress",
                "--depth",
                "1"
            ]
        );
    }

    #[test]
    fn submodule_progress_tracker_attributes_progress_and_finishes() {
        let (tx, rx) = channel();
        let mut tracker = SubmoduleProgressTracker::default();
        for line in [
            "Receiving objects: 10% (1/10)",
            "Submodule path 'components/foo': checked out",
            "Receiving objects: 50% (5/10)",
            "Cloning into '/abs/components/foo'...",
            "Cloning into '/abs/components/bar'...",
            "Submodule path 'bar': checked out",
            "Receiving objects: 100% (2/2), done.",
        ] {
            tracker.handle_line(line, &tx);
        }
        tracker.finish(&tx);
        tracker.finish(&tx);
        assert_eq!(
            drain(&rx),
            [
                "sub:components/foo:50",
                "sub-finish:components/foo",
                "sub-finish:foo",
                "sub:bar:100",
                "sub-finish:bar",
            ]
        );
    }

    #[test]
    fn resolve_actual_git_dir_for_repo_and_gitlink() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        assert_eq!(
            resolve_actual_git_dir(&repo, false).unwrap(),
            repo.join(".git")
        );

        let modules = tmp.path().join("modules").join("sub");
        let sub = tmp.path().join("sub");
        fs::create_dir_all(&modules).unwrap();
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join(".git"), "gitdir: ../modules/sub\n").unwrap();
        assert_eq!(
            resolve_actual_git_dir(&sub, true).unwrap(),
            modules.canonicalize().unwrap()
        );

        fs::write(sub.join(".git"), "nonsense").unwrap();
        assert_eq!(
            resolve_actual_git_dir(&sub, true).unwrap_err().to_string(),
            "Invalid gitlink file"
        );
    }

    #[test]
    fn initialize_modules_repo_writes_submodule_config() {
        let tmp = TempDir::new().unwrap();
        let modules_dir = tmp
            .path()
            .join(".git")
            .join("modules")
            .join("components/foo");
        let workdir = tmp.path().join("components").join("foo");
        initialize_modules_repo(&modules_dir, &workdir, "https://example.com/foo.git").unwrap();

        assert!(workdir.is_dir());
        assert_eq!(
            relative_worktree_path(&modules_dir, &workdir).unwrap(),
            Path::new("../../../../components/foo")
        );
        let config = fs::read_to_string(modules_dir.join("config")).unwrap();
        assert!(config.contains("bare = false"));
        assert!(config.contains("\tworktree = ../../../../components/foo\n"));
        assert!(config.contains("\turl = https://example.com/foo.git\n"));

        // A second call is a no-op once the config exists.
        initialize_modules_repo(&modules_dir, &workdir, "https://other.example").unwrap();
        assert_eq!(
            fs::read_to_string(modules_dir.join("config")).unwrap(),
            config
        );
    }

    #[cfg(unix)]
    mod local_repos {
        use super::*;
        use std::process::Command;

        fn git(dir: &Path, args: &[&str]) -> String {
            let output = Command::new("git")
                .args([
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "tag.gpgsign=false",
                    "-c",
                    "protocol.file.allow=always",
                ])
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }

        fn file_url(path: &Path) -> String {
            format!("file://{}", path.display())
        }

        /// Repo on branch `main` with two commits (`first.txt`, then `second.txt`) and
        /// tag `v1.0` on the first. Returns `(first_sha, second_sha)`.
        fn make_source_repo(dir: &Path) -> (String, String) {
            fs::create_dir_all(dir).unwrap();
            git(dir, &["init", "-q", "-b", "main"]);
            fs::write(dir.join("first.txt"), "first\n").unwrap();
            git(dir, &["add", "."]);
            git(dir, &["commit", "-q", "-m", "first"]);
            let first = git(dir, &["rev-parse", "HEAD"]);
            git(dir, &["tag", "v1.0"]);
            fs::write(dir.join("second.txt"), "second\n").unwrap();
            git(dir, &["add", "."]);
            git(dir, &["commit", "-q", "-m", "second"]);
            let second = git(dir, &["rev-parse", "HEAD"]);
            (first, second)
        }

        fn clone_opts(url: String, dest: &Path, reference: GitReference) -> CloneOptions {
            CloneOptions {
                url,
                path: dest.to_str().unwrap().to_string(),
                reference,
                recurse_submodules: false,
                shallow: true,
                mirror: None,
            }
        }

        #[test]
        fn clone_with_git_cli_checks_out_branch() {
            let tmp = TempDir::new().unwrap();
            let src = tmp.path().join("src");
            let (_, second) = make_source_repo(&src);
            let dest = tmp.path().join("dest");
            let (tx, rx) = channel();

            let opts = clone_opts(file_url(&src), &dest, GitReference::Branch("main".into()));
            assert_eq!(clone_with_git_cli(&opts, tx).unwrap(), dest);

            assert!(dest.join("second.txt").is_file());
            assert_eq!(git(&dest, &["rev-parse", "HEAD"]), second);
            let messages = drain(&rx);
            assert_eq!(messages.first().map(String::as_str), Some("update:0"));
            assert_eq!(messages.last().map(String::as_str), Some("update:100"));
            assert!(messages.contains(&"update:80".to_string()));
        }

        #[test]
        fn clone_with_git_cli_checks_out_older_commit() {
            let tmp = TempDir::new().unwrap();
            let src = tmp.path().join("src");
            let (first, _) = make_source_repo(&src);
            let dest = tmp.path().join("dest");
            let (tx, _rx) = channel();

            let opts = clone_opts(file_url(&src), &dest, GitReference::Commit(first.clone()));
            clone_with_git_cli(&opts, tx).unwrap();

            assert_eq!(git(&dest, &["rev-parse", "HEAD"]), first);
            assert!(!dest.join("second.txt").exists());
        }

        #[test]
        fn clone_with_git_cli_refuses_non_empty_destination() {
            let tmp = TempDir::new().unwrap();
            let dest = tmp.path().join("dest");
            fs::create_dir_all(&dest).unwrap();
            fs::write(dest.join("keep.txt"), "user data").unwrap();
            let (tx, _rx) = channel();

            let opts = clone_opts("file:///nonexistent".into(), &dest, GitReference::None);
            let err = clone_with_git_cli(&opts, tx).unwrap_err().to_string();
            assert!(err.contains("exists and is not empty"), "{err}");
            assert!(dest.join("keep.txt").is_file());
        }

        #[test]
        fn clone_with_git_cli_reports_git_diagnostics_on_failure() {
            let tmp = TempDir::new().unwrap();
            let dest = tmp.path().join("dest");
            let missing = file_url(&tmp.path().join("missing"));
            let (tx, _rx) = channel();

            let opts = clone_opts(missing.clone(), &dest, GitReference::None);
            let err = clone_with_git_cli(&opts, tx).unwrap_err().to_string();
            assert!(
                err.starts_with(&format!(
                    "git clone failed (exit code Some(128)) for {missing}: "
                )),
                "{err}"
            );
        }

        #[test]
        fn fetch_single_commit_git_cli_fetches_and_checks_out() {
            let tmp = TempDir::new().unwrap();
            let src = tmp.path().join("src");
            let (first, _) = make_source_repo(&src);
            let dest = tmp.path().join("dest");
            let (tx, rx) = channel();

            fetch_single_commit_git_cli(&dest, &file_url(&src), &first, &Some(tx), Some("sub"))
                .unwrap();

            assert_eq!(git(&dest, &["rev-parse", "HEAD"]), first);
            let messages = drain(&rx);
            assert_eq!(messages.first().map(String::as_str), Some("sub:sub:10"));
            assert_eq!(messages.last().map(String::as_str), Some("sub:sub:100"));
            assert!(messages.contains(&"sub:sub:80".to_string()));
        }

        #[test]
        fn clone_repository_checks_out_branch_tag_and_commit() {
            let tmp = TempDir::new().unwrap();
            let src = tmp.path().join("src");
            let (first, second) = make_source_repo(&src);

            for (name, reference, expected) in [
                ("branch", GitReference::Branch("main".into()), &second),
                ("tag", GitReference::Tag("v1.0".into()), &first),
                ("commit", GitReference::Commit(first.clone()), &first),
            ] {
                let dest = tmp.path().join(name);
                let (tx, rx) = channel();
                let path = clone_repository(clone_opts(file_url(&src), &dest, reference), tx)
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(path, dest);
                assert_eq!(&git(&dest, &["rev-parse", "HEAD"]), expected, "{name}");
                assert_eq!(dest.join("second.txt").exists(), expected == &second);
                assert_eq!(drain(&rx), ["update:50", "update:90", "finish"], "{name}");
            }
        }

        #[test]
        fn clone_repository_initializes_submodules() {
            let tmp = TempDir::new().unwrap();
            let lib = tmp.path().join("lib");
            let (_, lib_head) = make_source_repo(&lib);
            let parent = tmp.path().join("parent");
            make_source_repo(&parent);
            git(
                &parent,
                &["submodule", "add", "-q", &file_url(&lib), "components/lib"],
            );
            git(&parent, &["commit", "-q", "-m", "add submodule"]);

            let dest = tmp.path().join("dest");
            let (tx, rx) = channel();
            let mut opts = clone_opts(
                file_url(&parent),
                &dest,
                GitReference::Branch("main".into()),
            );
            opts.recurse_submodules = true;
            clone_repository(opts, tx).unwrap();

            let sub = dest.join("components/lib");
            assert!(sub.join("second.txt").is_file());
            assert!(fs::read_to_string(sub.join(".git"))
                .unwrap()
                .starts_with("gitdir: "));
            assert_eq!(git(&sub, &["rev-parse", "HEAD"]), lib_head);
            let messages = drain(&rx);
            assert!(messages.contains(&"sub-finish:components/lib".to_string()));
            assert_eq!(messages.last().map(String::as_str), Some("finish"));
        }

        #[test]
        fn fetch_single_commit_uses_gix_into_initialized_repo() {
            let tmp = TempDir::new().unwrap();
            let src = tmp.path().join("src");
            let (first, _) = make_source_repo(&src);
            let dest = tmp.path().join("dest");
            fs::create_dir_all(&dest).unwrap();
            git(&dest, &["init", "-q"]);
            let (tx, rx) = channel();

            fetch_single_commit(&dest, &file_url(&src), &first, Some(tx), None).unwrap();

            assert!(dest.join("first.txt").is_file());
            assert!(!dest.join("second.txt").exists());
            assert_eq!(
                fs::read_to_string(dest.join(".git/HEAD")).unwrap().trim(),
                first
            );
            assert_eq!(
                drain(&rx),
                [
                    "update:0",
                    "update:10",
                    "update:20",
                    "update:80",
                    "update:100"
                ]
            );
        }
    }
}
