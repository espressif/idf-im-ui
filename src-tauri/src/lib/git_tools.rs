use std::fs::{self, OpenOptions, read_to_string};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use log::{debug, error, info, trace, warn};
use gix::bstr::ByteSlice;
use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit};
use gix::refs::Target;
use anyhow::{anyhow, Result};

use std::io::Write;
use crate::command_executor::{ execute_command_with_dir, spawn_with_dir};
use crate::ensure_path;

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

    let checkout_result = execute_command_with_dir("git", &["checkout", commit_sha], dest_path.to_str().unwrap())?;

    if !checkout_result.status.success() {
        return Err(format!(
            "git checkout failed: {}",
            String::from_utf8_lossy(&checkout_result.stderr)
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
    fn send_progress(tx: &Option<Sender<ProgressMessage>>, submodule_name: Option<&str>, progress: u8) {
        if let (Some(tx), Some(submodule_name)) = (tx, submodule_name) {
            let _ = tx.send(ProgressMessage::SubmoduleUpdate( (submodule_name.to_string(), progress as u64) ));
        }
    }

    if !dest_path.join(".git").exists() {
        std::fs::create_dir_all(dest_path)?;

        let init_result = execute_command_with_dir("git", &["init"], dest_path.to_str().unwrap())?;

        if !init_result.status.success() {
            return Err(format!(
                "git init failed: {}",
                String::from_utf8_lossy(&init_result.stderr)
            ).into());
        }

        let remote_result = execute_command_with_dir("git", &["remote", "add", "origin", url], dest_path.to_str().unwrap())?;

        if !remote_result.status.success() {
            return Err(format!(
                "git remote add failed: {}",
                String::from_utf8_lossy(&remote_result.stderr)
            ).into());
        }
    }

    // 10% - Initialized
    send_progress(tx, submodule_name, 10);

    // Fetch with progress parsing from stderr
    let mut child = spawn_with_dir("git", &["fetch", "--depth", "1", "--progress", "origin", commit_sha], dest_path.to_str().unwrap())?;

    // Parse progress from git's stderr
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            // Git outputs progress like: "Receiving objects:  45% (123/456)"
            if let Some(percentage) = parse_git_progress(&line) {
                // Scale git's 0-100% to our 10-80% range
                let scaled = 10 + ((percentage as u8) * 70 / 100);
                send_progress(tx, submodule_name, scaled);
            }
        }
    }

    let status = child.wait()?;

    if !status.success() {
        // Fallback: fetch default branch
        debug!("Direct SHA fetch failed, trying default branch");

        let fallback_result = execute_command_with_dir("git", &["fetch", "--depth", "1", "origin"], dest_path.to_str().unwrap())?;

        if !fallback_result.status.success() {
            return Err(format!(
                "git fetch failed: {}",
                String::from_utf8_lossy(&fallback_result.stderr)
            ).into());
        }
    }

    // 80% - Fetched
    send_progress(tx, submodule_name, 80);

    // Checkout
    checkout_with_git_cli(dest_path, commit_sha)?;

    // 100% - Complete
    send_progress(tx, submodule_name, 100);

    Ok(())
}

/// Parses a line of `git fetch` stderr output to extract a progress percentage.
///
/// Git's progress output can have a few different formats, such as:
/// - "Receiving objects:  45% (123/456)"
/// - "Resolving deltas: 100% (12/12), done."
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

/// A public test wrapper for the private `parse_git_progress` function.
///
/// This function exposes the functionality of `parse_git_progress` for use in tests
/// or other modules, without making the original function public.
///
/// # Arguments
///
/// * `line` - A string slice representing a single line from git's stderr.
///
/// # Returns
///
/// * The result of `parse_git_progress(line)`.
pub fn parse_git_progress_test(line: &str) -> Option<u64> {
    parse_git_progress(line)
}

/// Clones or updates a git submodule to a specific commit with progress reporting.
///
/// This function uses the `git` command-line tool to perform the operations. It initializes
/// the submodule repository if it doesn't exist, fetches the specific commit with a shallow
/// depth, and then checks it out. Progress is reported through the provided sender.
///
/// # Arguments
///
/// * `submodule_name` - The name of the submodule.
/// * `url` - The URL of the submodule's repository.
/// * `commit_sha` - The commit SHA to check out.
/// * `dest_path` - The local path where the submodule should be cloned/updated.
/// * `tx` - A sender for reporting progress as `(String, u8)` tuples (submodule name, percentage).
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err` if any of the git operations fail.
pub fn clone_or_update_submodule(
    submodule_name: &str,
    url: &str,
    commit_sha: &str,
    dest_path: &Path,
    tx: std::sync::mpsc::Sender<(String, u8)>,
) -> Result<(), Box<dyn std::error::Error>> {
    fn send_progress(
        tx: &std::sync::mpsc::Sender<(String, u8)>,
        submodule_name: &str,
        progress: u8,
    ) {
        let _ = tx.send((submodule_name.to_string(), progress));
    }

    if !dest_path.join(".git").exists() {
        std::fs::create_dir_all(dest_path)?;

        let init_result = execute_command_with_dir("git", &["init"], dest_path.to_str().unwrap())?;

        if !init_result.status.success() {
            return Err(format!(
                "git init failed: {}",
                String::from_utf8_lossy(&init_result.stderr)
            ).into());
        }

        let remote_result = execute_command_with_dir("git", &["remote", "add", "origin", url], dest_path.to_str().unwrap())?;

        if !remote_result.status.success() {
            return Err(format!(
                "git remote add failed: {}",
                String::from_utf8_lossy(&remote_result.stderr)
            ).into());
        }
    }

    // 10% - Initialized
    send_progress(&tx, submodule_name, 10);

    // Fetch with progress parsing from stderr
    let mut child = spawn_with_dir("git", &["fetch", "--depth", "1", "--progress", "origin", commit_sha], dest_path.to_str().unwrap())?;

    // Parse progress from git's stderr
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            // Git outputs progress like: "Receiving objects:  45% (123/456)"
            if let Some(percentage) = parse_git_progress(&line) {
                // Scale git's 0-100% to our 10-80% range
                let scaled = 10 + ((percentage as u8) * 70 / 100);
                send_progress(&tx, submodule_name, scaled);
            }
        }
    }

    let status = child.wait()?;

    if !status.success() {
        // Fallback: fetch default branch
        debug!("Direct SHA fetch failed, trying default branch");

        let fallback_result = execute_command_with_dir("git", &["fetch", "--depth", "1", "origin"], dest_path.to_str().unwrap())?;

        if !fallback_result.status.success() {
            return Err(format!(
                "git fetch failed: {}",
                String::from_utf8_lossy(&fallback_result.stderr)
            ).into());
        }
    }

    // 80% - Fetched
    send_progress(&tx, submodule_name, 80);

    // Checkout
    checkout_with_git_cli(dest_path, commit_sha)?;

    // 100% - Complete
    send_progress(&tx, submodule_name, 100);

    Ok(())
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

    // Configure shallow clone
    let shallow = if options.shallow {
        match &options.reference {
            GitReference::Commit(_) => gix::remote::fetch::Shallow::NoChange,
            _ => gix::remote::fetch::Shallow::DepthAtRemote(
                std::num::NonZeroU32::new(1).unwrap()
            ),
        }
    } else {
        gix::remote::fetch::Shallow::NoChange
    };

    let should_interrupt = &AtomicBool::new(false);
    let progress = gix::progress::Discard;

    // Prepare clone
    let url = gix::url::parse(options.url.as_str().into())?;

    let mut prepare = gix::prepare_clone(url, &dest_path)?
        .with_remote_name("origin")?
        .with_shallow(shallow);

    // Fetch
    info!("Cloning repository from {:?}", options);
    let (mut checkout, _) = match prepare
        .fetch_then_checkout(gix::progress::Discard, should_interrupt){
            Ok(res) => res,
            Err(e) => {
                let _ = tx.send(ProgressMessage::Finish);
                error!("Failed to fetch repository: {}", e);
                return Err(Box::new(e));
            }
        };

    let _ = tx.send(ProgressMessage::Update(50));

    // Checkout
    let (repo, _) = match checkout
        .main_worktree(progress, should_interrupt){
            Ok(res) => res,
            Err(e) => {
                let _ = tx.send(ProgressMessage::Finish);
                error!("Failed to checkout repository: {}", e);
                return Err(Box::new(e));
            }
        };

    // Checkout specific reference
    match checkout_reference(&repo, &options.reference) {
        Ok(_) => {
            let _ = tx.send(ProgressMessage::Update(90));
        }
        Err(e) => {
            let _ = tx.send(ProgressMessage::Finish);
            error!("Failed to checkout reference: {}", e);
        }
    }

    info!("Cloned repository to {} proceeding to submodules...", dest_path.display());

    // Handle submodules
    info!("Starting submodule update...");
    if options.recurse_submodules {
        info!("Recurse submodules is TRUE");
        match update_submodules_shallow(&repo, tx.clone()) {
            Ok(_) => info!("Submodules updated successfully"),
            Err(e) => error!("Submodule update failed: {}", e),
        }
    }

    Ok(dest_path)
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
) -> Result<(), Box<dyn std::error::Error>> {

    let workdir = repo.work_dir()
        .ok_or("Repository has no working directory")?;

    let gitmodules_path = workdir.join(".gitmodules");
    if !gitmodules_path.exists() {
        info!("No .gitmodules file found, skipping submodule initialization");
        return Ok(());
    }

    info!(".gitmodules found at {}", gitmodules_path.display());

    let submodules = match repo.submodules()? {
        Some(subs) => subs,
        None => {
            info!("No submodules configured");
            return Ok(());
        }
    };

    let git_dir = repo.git_dir();

    // Get the parent repository's remote URL for resolving relative URLs
    let parent_url = get_remote_url(repo)?;
    info!("Parent repository URL: {}", parent_url);

    // Get HEAD tree to find submodule commit SHAs
    let head_commit = repo.head_commit()?;
    let tree = head_commit.tree()?;

    // Build a map of submodule paths to their commit OIDs
    let mut submodule_commits = std::collections::HashMap::new();
    collect_submodule_commits(&tree, "", &mut submodule_commits)?;

    info!("Found {} submodule commit entries in tree", submodule_commits.len());

    for submodule in submodules {
        let name = submodule.name().to_string();
        let path = submodule.path()?.to_string();
        let url_raw = submodule.url()?.to_bstring().to_string();

        // Resolve relative URLs
        let url = resolve_submodule_url(&url_raw, &parent_url)?;

        info!("Processing submodule: {} at path: {}", name, path);
        if url != url_raw {
            info!("  Resolved URL: {} -> {}", url_raw, url);
        }

        let _ = tx.send(ProgressMessage::SubmoduleUpdate((name.clone(), 0)));

        // Get the expected commit SHA
        let expected_sha = match submodule_commits.get(&path) {
            Some(oid) => oid.to_string(),
            None => {
                warn!("No commit entry found for submodule {} at path {}", name, path);
                let _ = tx.send(ProgressMessage::SubmoduleFinish(name.clone()));
                continue;
            }
        };

        info!("Submodule {} should be at commit {}", name, &expected_sha[..7]);

        let submodule_dir = workdir.join(&path);
        let modules_dir = git_dir.join("modules").join(&path);

        // Step 1: Add to .git/config
        add_submodule_to_config(&git_dir.join("config"), &name, &path, &url)?;

        // Step 2: Initialize .git/modules/<path>
        initialize_modules_repo(&modules_dir, &submodule_dir, &url)?;

        // Step 3: Create submodule workdir and gitlink files
        std::fs::create_dir_all(&submodule_dir)?;
        create_gitlink(&submodule_dir, &git_dir, &path)?;

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
            let _ = update_submodules_shallow(&sub_repo, tx.clone());
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
            let url = remote.url(gix::remote::Direction::Fetch)
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
    let base_url = url::Url::parse(base)
        .map_err(|e| format!("Failed to parse base URL: {}", e))?;

    // Get the path without the .git extension and repo name
    let mut path_segments: Vec<&str> = base_url
        .path_segments()
        .ok_or("Base URL has no path")?
        .collect();

    // Remove the repository name (last segment)
    if !path_segments.is_empty() {
        path_segments.pop();
    }

    // Process relative path
    for segment in relative.split('/') {
        match segment {
            "." => continue,
            ".." => {
                path_segments.pop();
            }
            "" => continue,
            s => path_segments.push(s),
        }
    }

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
    let path_part = parts[1];  // e.g., "user/repo.git"

    // Remove .git extension and split path
    let path_clean = path_part.trim_end_matches(".git");
    let mut path_segments: Vec<&str> = path_clean.split('/').collect();

    // Remove repository name
    if !path_segments.is_empty() {
        path_segments.pop();
    }

    // Process relative path
    for segment in relative.split('/') {
        match segment {
            "." => continue,
            ".." => {
                path_segments.pop();
            }
            "" => continue,
            s => path_segments.push(s),
        }
    }

    let new_path = path_segments.join("/");
    Ok(format!("{}:{}", host_part, new_path))
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
        let entry = entry?;
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
        debug!("Modules repo already initialized at {}", modules_dir.display());
        return Ok(());
    }

    // Ensure parent directory exists
    if let Some(parent) = modules_dir.parent() {
        info!("Creating parent directories: {}", parent.display());
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent dir {}: {}", parent.display(), e))?;
    }

    // Create modules_dir itself if it doesn't exist
    if !modules_dir.exists() {
        info!("Creating modules dir: {}", modules_dir.display());
        fs::create_dir_all(modules_dir)
            .map_err(|e| format!("Failed to create modules dir {}: {}", modules_dir.display(), e))?;
    }

    // Check what's in the directory before init
    info!("Contents of {} before init: {:?}", modules_dir.display(), fs::read_dir(modules_dir)?.collect::<Vec<_>>());

    // Initialize with gix - try to open first, if it fails, then init
    let repo = match gix::open(modules_dir) {
        Ok(repo) => {
            info!("Repository already exists at {}", modules_dir.display());
            repo
        }
        Err(_) => {
            info!("Initializing new git repo at {}", modules_dir.display());
            gix::init_bare(modules_dir)
                .map_err(|e| format!("Failed to init git repo at {}: {}", modules_dir.display(), e))?
        }
    };

    // Use the repository's git_dir (might be different from modules_dir)
    let git_dir = repo.git_dir();
    info!("Git dir is at: {}", git_dir.display());

    let config_path = git_dir.join("config");

    if !config_path.exists() {
        return Err(format!("Config file not found at {}", config_path.display()).into());
    }

    // Get absolute path to workdir
    let workdir_abs = if submodule_workdir.is_absolute() {
        submodule_workdir.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_default()
            .join(submodule_workdir)
    };

    // Read and update config
    let mut config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config at {}: {}", config_path.display(), e))?;

    // For a bare repo, change it to non-bare and add worktree
    if config_content.contains("bare = true") {
        config_content = config_content.replace("bare = true", "bare = false");
    }

    // Add worktree
    if !config_content.contains("worktree") {
        if let Some(pos) = config_content.find("[core]") {
            if let Some(end_pos) = config_content[pos..].find('\n') {
                let insert_pos = pos + end_pos + 1;
                config_content.insert_str(
                    insert_pos,
                    &format!("\tworktree = {}\n", workdir_abs.display())
                );
            }
        }
    }

    // Add remote
    if !config_content.contains("[remote \"origin\"]") {
        config_content.push_str(&format!(
            "\n[remote \"origin\"]\n\
             \turl = {}\n\
             \tfetch = +refs/heads/*:refs/remotes/origin/*\n",
            url
        ));
    }

    fs::write(&config_path, config_content)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    info!("✓ Initialized modules repo: {}", modules_dir.display());
    Ok(())
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

    // Calculate the relative path from submodule workdir to .git/modules/<path>
    // We need to go up from the submodule dir to the repo root, then into .git/modules/<path>

    // Count directory depth of submodule path
    let path_components: Vec<&str> = submodule_path.split('/').collect();
    let depth = path_components.len();

    // Build relative path: ../ for each component + .git/modules/<path>
    let mut relative_path = String::new();
    for _ in 0..depth {
        relative_path.push_str("../");
    }
    relative_path.push_str(".git/modules/");
    relative_path.push_str(submodule_path);

    let gitlink_content = format!("gitdir: {}\n", relative_path);

    fs::write(&gitlink_path, gitlink_content.clone())?;

    debug!("Created gitlink at {}: {}", gitlink_path.display(), gitlink_content.trim());

    // CRITICAL: Also create the reverse link (gitdir file in modules dir)
    let modules_dir = parent_git_dir.join("modules").join(submodule_path);
    let gitdir_file = modules_dir.join("gitdir");

    // This should be an absolute path or relative path to the workdir
    let workdir_path = submodule_dir.canonicalize()
        .unwrap_or_else(|_| submodule_dir.to_path_buf());

    fs::write(&gitdir_file, format!("{}\n", workdir_path.display()))?;

    debug!("Created gitdir link at {}", gitdir_file.display());

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
    let repo = gix::open(modules_dir)
        .or_else(|_| gix::init(modules_dir))?;

    let expected_oid = gix::ObjectId::from_hex(commit_sha.as_bytes())
        .map_err(|e| format!("Invalid SHA '{}': {}", commit_sha, e))?;

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
    let remote_url = gix::url::parse(url.into())
        .map_err(|e| format!("Invalid URL '{}': {}", url, e))?;

    let remote = repo
        .remote_at(remote_url)?;

    let connection = remote
        .connect(gix::remote::Direction::Fetch)
        .map_err(|e| format!("Failed to connect: {}", e))?;

    send_progress(&tx, submodule_name, 40);

    let shallow = gix::remote::fetch::Shallow::DepthAtRemote(NonZeroU32::new(1).unwrap());

    let _outcome = connection
        .prepare_fetch(gix::progress::Discard, gix::remote::ref_map::Options::default())
        .map_err(|e| format!("Failed to prepare fetch: {}", e))?
        .with_shallow(shallow)
        .receive(gix::progress::Discard, &AtomicBool::new(false))
        .map_err(|e| format!("Failed to receive: {}", e))?;

    send_progress(&tx, submodule_name, 70);

    // Verify commit exists
    repo.find_commit(expected_oid)
        .map_err(|_| format!("Commit {} not found after fetch", &commit_sha[..7]))?;

    // Update HEAD to point to this commit (detached state)
    std::fs::write(modules_dir.join("HEAD"), format!("{}\n", commit_sha))?;

    debug!("Fetched commit {} to modules dir", &commit_sha[..7]);
    Ok(())
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

    let commit_oid = gix::ObjectId::from_hex(commit_sha.as_bytes())?;
    let commit = repo.find_commit(commit_oid)?;
    let tree = commit.tree()?;

    // Simple approach: Just checkout files, don't worry about index for now
    // Git will rebuild it when needed
    checkout_tree_recursive(&repo, &tree, submodule_workdir)?;

    debug!("Checked out files to {}", submodule_workdir.display());
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
        let entry = entry?;
        let entry_mode = entry.mode();
        let entry_oid = entry.oid();
        let entry_name = entry.filename();

        let target_path = target_dir.join(entry_name.to_path_lossy().as_ref());

        if entry_mode.is_tree() {
            // Create directory and recurse
            fs::create_dir_all(&target_path)?;

            let subtree = repo.find_tree(entry_oid)?;
            checkout_tree_recursive(repo, &subtree, &target_path)?;

        } else if entry_mode.is_blob() || entry_mode.is_executable() {
            // Ensure parent directory exists
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Write blob content
            let object = repo.find_object(entry.oid())?;
            let blob = object.try_into_blob()?;
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
            debug!("Successfully fetched {} using gix", &commit_sha[..7.min(commit_sha.len())]);
            send_progress(&tx, submodule_name, 100);
            Ok(())
        }
        Err(e) => {
            debug!(
                "gix fetch failed ({}), falling back to git CLI for {}",
                e,
                &commit_sha[..7.min(commit_sha.len())]
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
    let expected_oid = gix::ObjectId::from_hex(commit_sha.as_bytes())
        .map_err(|e| format!("Invalid SHA '{}': {}", commit_sha, e))?;

    // For submodules, we need to handle the gitlink file
    let git_path = dest_path.join(".git");
    let is_submodule = git_path.is_file(); // Submodules have .git as a file, not a directory

    // Determine the actual git directory
    // Determine the actual git directory
    let actual_git_dir = if is_submodule {
        // Read the gitlink file to find the actual .git directory
        let gitlink_content = fs::read_to_string(&git_path)?;
        let git_dir_path = gitlink_content
            .trim()
            .strip_prefix("gitdir: ")
            .ok_or("Invalid gitlink file")?;

        // Resolve relative path
        dest_path.join(git_dir_path).canonicalize()?
    } else {
        dest_path.join(".git")
    };

    // Check if we already have this commit
    if actual_git_dir.exists() {
        if let Ok(repo) = gix::open(dest_path) {
            if repo.find_commit(expected_oid).is_ok() {
                debug!(
                    "Commit {} already exists locally",
                    &commit_sha[..7.min(commit_sha.len())]
                );
                send_progress(tx, submodule_name, 80);
                checkout_commit_gix(&repo, expected_oid)?;
                return Ok(());
            }
        }
    }

    // Initialize repository if needed
    let repo = if !actual_git_dir.exists() {
        fs::create_dir_all(&actual_git_dir)?;

        if is_submodule {
            // For submodules, initialize in the modules directory
            gix::init(&actual_git_dir)?;

            // Create HEAD file
            fs::write(actual_git_dir.join("HEAD"), "ref: refs/heads/master\n")?;

            // Open the repository
            gix::open(dest_path)?
        } else {
            gix::init(dest_path)?
        }
    } else {
        gix::open(dest_path)?
    };

    send_progress(tx, submodule_name, 10);

    // Parse the remote URL
    let remote_url = gix::url::parse(url.into())
        .map_err(|e| format!("Invalid URL '{}': {}", url, e))?;

    // Create remote with the specific SHA as a refspec
    let remote = repo
        .remote_at(remote_url)?
        .with_fetch_tags(gix::remote::fetch::Tags::None)
        .with_refspecs(
            [commit_sha].into_iter(),
            gix::remote::Direction::Fetch,
        )
        .map_err(|e| format!("Failed to set refspec: {}", e))?;

    // Connect to the remote
    let connection = remote
        .connect(gix::remote::Direction::Fetch)
        .map_err(|e| format!("Failed to connect: {}", e))?;

    send_progress(tx, submodule_name, 20);

    // Configure shallow fetch
    let shallow = gix::remote::fetch::Shallow::DepthAtRemote(
        NonZeroU32::new(1).unwrap()
    );

    // Prepare and execute the fetch
    let outcome = connection
        .prepare_fetch(gix::progress::Discard, gix::remote::ref_map::Options::default())
        .map_err(|e| format!("Failed to prepare fetch: {}", e))?
        .with_shallow(shallow)
        .receive(gix::progress::Discard, should_interrupt)
        .map_err(|e| format!("Failed to receive: {}", e))?;

    send_progress(tx, submodule_name, 80);

    trace!(
        "Fetch complete: {} ref mappings",
        outcome.ref_map.mappings.len()
    );

    // Verify the commit exists
    let _commit = repo.find_commit(expected_oid).map_err(|_| {
        format!(
            "Commit {} not found after fetch",
            &commit_sha[..7.min(commit_sha.len())]
        )
    })?;

    // Checkout using gix
    checkout_commit_gix(&repo, expected_oid)?;

    Ok(())
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

    let commit = repo.find_commit(commit_oid)?;
    let tree = commit.tree()?;

    let worktree = repo.work_dir()
        .ok_or("Repository has no working directory")?;

    // Walk the tree and checkout each file
    for entry in tree.iter() {
        let entry = entry?;
        let path = worktree.join(entry.repo.path());

        if entry.mode().is_tree() {
            // Create directory
            fs::create_dir_all(&path)?;
        } else if entry.mode().is_blob() || entry.mode().is_executable() {
            // Create parent directory
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Get blob content
            let object = repo.find_object(entry.oid())?;
            let blob = object.try_into_blob()?;

            // Write file
            fs::write(&path, blob.data.clone())?;

            // Set executable bit if needed
            #[cfg(unix)]
            if entry.mode().is_executable() {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms)?;
            }
        }
    }

    // Update HEAD
    let git_dir = repo.git_dir();
    fs::write(git_dir.join("HEAD"), format!("{}\n", commit_oid))?;

    Ok(())
}

/// Defines the type of Git reference to be checked out.
#[derive(Debug)]
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
        GitReference::Branch(branch) => {
            info!("Checking out branch: {}", branch);

            let refname = format!("refs/remotes/origin/{}", branch);
            let mut  git_ref = match repo.find_reference(&refname){
              Ok(r) => r,
              Err(_) => repo.find_reference(&format!("refs/heads/{}", branch))?,
            };

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

            // Set HEAD
            set_head_to_ref(repo, &local_refname, &format!("checkout: moving to {}", branch))?;
        }
        GitReference::Tag(tag) => {
            info!("Checking out tag: {}", tag);

            let refname = format!("refs/tags/{}", tag);
            let mut git_ref = repo.find_reference(&refname)?;

            let commit = git_ref.peel_to_commit()?;

            set_head_detached(repo, commit.id(), &format!("checkout: moving to {}", &commit.id()))?;

        }
        GitReference::Commit(commit_id) => {
            info!("Checking out commit: {}", commit_id);

            let oid = gix::ObjectId::from_hex(commit_id.as_bytes())?;

            let commit = repo.find_commit(oid)?;
            set_head_detached(repo, commit.id(), &format!("checkout: moving to {}", &commit.id()))?;
        }
        GitReference::None => {
            debug!("Using default reference from clone");
        }
    }

    Ok(())
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

    let edit = RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: message.into(),
            },
            expected: PreviousValue::Any,
            new: Target::Object(commit_id.detach()),  // For detached HEAD
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

    let edit = RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: message.into(),
            },
            expected: PreviousValue::Any,
            new: Target::Symbolic(gix::refs::FullName::try_from(refname)?),
        },
        name: gix::refs::FullName::try_from("HEAD")?,
        deref: false,
    };

    repo.edit_reference(edit)?;
    Ok(())
}

/// Clones the ESP-IDF repository with specified options.
///
/// This is a high-level function that orchestrates the cloning of ESP-IDF. It determines
/// the correct repository URL based on mirror settings, parses the desired version into a
/// `GitReference`, and then calls `clone_repository` to perform the actual clone operation.
///
/// # Arguments
///
/// * `path` - The local filesystem path where the repository should be cloned.
/// * `repository` - An optional repository string (e.g., "espressif/esp-idf").
/// * `version` - The version to check out (can be a branch, tag, or commit SHA).
/// * `mirror` - An optional mirror URL prefix.
/// * `with_submodules` - If `true`, submodules will be initialized and updated.
/// * `tx` - A sender for reporting clone progress.
///
/// # Returns
///
/// * `Ok(String)` with the path to the cloned repository on success.
/// * `Err(String)` with an error message on failure.
pub fn get_esp_idf(
    path: &str,
    repository: Option<&str>,
    version: &str,
    mirror: Option<&str>,
    with_submodules: bool,
    tx: Sender<ProgressMessage>,
) -> Result<String, String> {
    // Ensure the path exists
    let _ = ensure_path(path);

    let url = get_repo_url(repository, mirror);

    let shallow = true;
    // Parse version into a GitReference
    let reference = if version == "master" {
        GitReference::Branch("master".to_string())
    } else if version.contains("release")  {
        GitReference::Branch(version.to_string().replace("release-", "release/"))
    } else if version.len() == 40 && version.chars().all(|c| c.is_ascii_hexdigit()) {
        // If version is a 40-character hex string, assume it's a commit hash
        GitReference::Commit(version.to_string())
    } else {
        // Otherwise assume it's a tag
        GitReference::Tag(version.to_string())
    };

    let clone_options = CloneOptions {
        url,
        path: path.to_string(),
        reference,
        recurse_submodules: with_submodules,
        shallow: shallow, // Default to shallow clone when possible
    };

    match clone_repository(clone_options, tx) {
        Ok(repo) => Ok(repo.to_str().unwrap_or(path).to_string()),
        Err(e) => Err(e.to_string()),
    }
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
pub fn get_repo_url(
    repository: Option<&str>,
    mirror: Option<&str>,
) -> String {
    // Determine the repository URL
    let repo_part_url = match repository {
        Some(repo) => format!("{}.git", repo),
        None => {
            if mirror.map_or(false, |m| m.contains("https://gitee.com/")) {
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
            if mirror.map_or(false, |m| m.contains("https://gitee.com/")) {
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
            format!("{}/{}/raw/{}/{}", mirror_url, repo_name, ref_name, file_path)
        } else if mirror_url.contains("gitlab") {
            // GitLab raw format: https://gitlab.com/owner/repo/-/raw/branch/path
            format!("{}/{}/-/raw/{}/{}", mirror_url, repo_name, ref_name, file_path)
        } else {
            // Generic git hosting - try GitHub format
            format!("{}/{}/raw/{}/{}", mirror_url, repo_name, ref_name, file_path)
        }
    } else {
        // Default to GitHub raw format: https://raw.githubusercontent.com/owner/repo/branch/path
        format!("https://raw.githubusercontent.com/{}/{}/{}", repo_name, ref_name, file_path)
    }
}
