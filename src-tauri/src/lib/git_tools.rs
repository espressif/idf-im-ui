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

/// Use git CLI to checkout a commit (fast operation, always works)
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

/// Messages that can be sent to update the progress bar.
pub enum ProgressMessage {
    /// Update the progress bar with the given value (percentage 0-100).
    Update(u64),
    /// Finish the progress bar.
    Finish,
    /// Update submodule progress: (name, percentage 0-100)
    SubmoduleUpdate((String, u64)),
    /// Submodule finished
    SubmoduleFinish(String),
}

/// Fallback: Fetch using git CLI with real progress parsing
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

/// Parse git progress output to extract percentage
///
/// Returns Some(percentage) if a progress percentage is found, None otherwise
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

/// Public function for testing parse_git_progress functionality
pub fn parse_git_progress_test(line: &str) -> Option<u64> {
    parse_git_progress(line)
}

/// Clone or update a git submodule with progress reporting
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




/// Helper to send progress updates
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

/// Update submodules fetching ONLY the specific commit SHA with progress reporting
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

/// Get the remote URL of the repository
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

/// Resolve a potentially relative submodule URL against the parent URL
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

/// Resolve relative URL for HTTP(S) URLs
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

/// Resolve relative URL for SSH URLs (git@host:path format)
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

/// Recursively collect all submodule commits from the tree
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

/// Initialize the repository structure in .git/modules/<path>
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

/// Create the gitlink file in the submodule's working directory
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

/// Fetch a commit into the modules directory
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
        .remote_at(remote_url)?
        .with_fetch_tags(gix::remote::fetch::Tags::None)
        .with_refspecs(
            [commit_sha].into_iter(),
            gix::remote::Direction::Fetch,
        )
        .map_err(|e| format!("Failed to set refspec: {}", e))?;

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

/// Checkout files from modules repo to the submodule's working directory
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

/// Recursively checkout a tree
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
            let object = repo.find_object(entry_oid)?;
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

/// Add submodule configuration to parent's .git/config
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

/// Fetch a single commit with depth=1 and progress reporting (updated for submodules)
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

/// Fetch using gix with milestone-based progress reporting (updated for submodules)
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

/// Checkout a commit using gix (pure Rust)
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

/// Use git CLI to checkout a commit (fast operation, always works)


#[derive(Debug)]
pub enum GitReference {
    Branch(String),
    Tag(String),
    Commit(String),
    None,
}

#[derive(Debug)]
pub struct CloneOptions {
    pub url: String,
    pub path: String,
    pub reference: GitReference,
    pub recurse_submodules: bool,
    pub shallow: bool,
}

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

/// Get ESP-IDF repository by version and mirror
///
/// # Arguments
/// * `path` - Path where to clone the repository
/// * `repository` - Optional repository name pair (e.g. "espressif/esp-idf")
/// * `version` - Version to checkout (tag or commit or 'master')
/// * `mirror` - Optional mirror URL
/// * `with_submodules` - Whether to also clone submodules
/// * `tx` - Sender for progress reporting
///
/// # Returns
/// * `Result<String, git2::Error>` - Repository path or an error
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
