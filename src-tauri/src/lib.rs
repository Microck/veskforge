use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    env,
    ffi::{OsStr, OsString},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

const VENCORD_REPO: &str = "https://github.com/Vendicated/Vencord.git";
const MANIFEST_FILE: &str = "manifest.json";
const REQUIRED_DIST_FILES: [&str; 5] = [
    "package.json",
    "vencordDesktopMain.js",
    "vencordDesktopPreload.js",
    "vencordDesktopRenderer.js",
    "vencordDesktopRenderer.css",
];
const PLUGIN_ENTRYPOINT_FILES: [&str; 4] = ["index.ts", "index.tsx", "index.js", "index.jsx"];
const PLUGIN_FILE_EXTENSIONS: [&str; 4] = ["ts", "tsx", "js", "jsx"];
const IGNORED_PLUGIN_DISCOVERY_DIRS: [&str; 8] = [
    ".git",
    "node_modules",
    "dist",
    "build",
    "target",
    ".next",
    ".turbo",
    "coverage",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginRecord {
    id: String,
    name: String,
    source: PluginSource,
    enabled: bool,
    installed_path: String,
    git_ref: Option<String>,
    last_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum PluginSource {
    LocalFile {
        path: String,
    },
    LocalFolder {
        path: String,
    },
    Git {
        url: String,
        reference: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePolicy {
    mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    update_policy: UpdatePolicy,
    plugins: Vec<PluginRecord>,
    last_successful_build: Option<BuildMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildMetadata {
    built_at: String,
    dist_path: String,
    vencord_revision: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvironmentStatus {
    app_data_dir: String,
    workspace_dir: String,
    vencord_dir: String,
    dist_dir: String,
    vesktop_state_candidates: Vec<String>,
    selected_vesktop_state: Option<String>,
    tools: Vec<ToolStatus>,
    manifest: Manifest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolStatus {
    name: String,
    available: bool,
    version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandResult {
    ok: bool,
    message: String,
    log: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddPluginRequest {
    source: PluginSource,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetUpdatePolicyRequest {
    mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyRequest {
    state_path: Option<String>,
}

fn default_manifest() -> Manifest {
    Manifest {
        update_policy: UpdatePolicy {
            mode: "manual".to_string(),
        },
        plugins: Vec::new(),
        last_successful_build: None,
    }
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Could not resolve app data dir: {err}"))?;
    fs::create_dir_all(&dir).map_err(|err| format!("Could not create app data dir: {err}"))?;
    Ok(dir)
}

fn workspace_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_data_dir(app)?.join("workspace");
    fs::create_dir_all(&dir).map_err(|err| format!("Could not create workspace dir: {err}"))?;
    Ok(dir)
}

fn managed_plugins_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_data_dir(app)?.join("plugins");
    fs::create_dir_all(&dir).map_err(|err| format!("Could not create plugin store: {err}"))?;
    Ok(dir)
}

fn vencord_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(workspace_dir(app)?.join("Vencord"))
}

fn dist_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(vencord_dir(app)?.join("dist"))
}

fn manifest_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join(MANIFEST_FILE))
}

fn read_manifest(app: &AppHandle) -> Result<Manifest, String> {
    let path = manifest_path(app)?;
    if !path.exists() {
        return Ok(default_manifest());
    }

    let content = fs::read_to_string(&path)
        .map_err(|err| format!("Could not read manifest {}: {err}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|err| format!("Could not parse manifest {}: {err}", path.display()))
}

fn write_manifest(app: &AppHandle, manifest: &Manifest) -> Result<(), String> {
    let path = manifest_path(app)?;
    let content = serde_json::to_string_pretty(manifest)
        .map_err(|err| format!("Could not serialize manifest: {err}"))?;
    fs::write(&path, content)
        .map_err(|err| format!("Could not write manifest {}: {err}", path.display()))
}

fn now_stamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn sanitize_id(input: &str) -> String {
    let mut id = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    let trimmed = id.trim_matches('-');
    if trimmed.is_empty() {
        format!("plugin-{}", now_stamp())
    } else {
        trimmed.to_string()
    }
}

fn plugin_name_from_source(source: &PluginSource, fallback: Option<String>) -> String {
    if let Some(name) = fallback {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    match source {
        PluginSource::LocalFile { path } | PluginSource::LocalFolder { path } => Path::new(path)
            .file_stem()
            .or_else(|| Path::new(path).file_name())
            .and_then(OsStr::to_str)
            .unwrap_or("custom-plugin")
            .to_string(),
        PluginSource::Git { url, .. } => url
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("git-plugin")
            .to_string(),
    }
}

fn normalize_github_plugin_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://github.com/")
        .ok_or_else(|| "Git plugin sources must be HTTPS GitHub repository URLs.".to_string())?;

    if without_scheme.contains("/blob/") || without_scheme.contains("/tree/") {
        return Err(
            "Git plugin sources must point at a repository root, not a GitHub file or folder URL."
                .to_string(),
        );
    }
    if trimmed.starts_with("https://raw.githubusercontent.com/") {
        return Err(
            "Raw GitHub file URLs are not valid Git plugin sources. Add the repository URL instead."
                .to_string(),
        );
    }

    let parts = without_scheme
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .split('/')
        .collect::<Vec<_>>();
    if parts.len() != 2 || parts.iter().any(|part| part.trim().is_empty()) {
        return Err("Git plugin sources must look like https://github.com/owner/repo.".to_string());
    }

    Ok(format!("https://github.com/{}/{}.git", parts[0], parts[1]))
}

fn validate_local_plugin_path(path: &Path) -> Result<(), String> {
    if path.is_file() {
        let ext = path.extension().and_then(OsStr::to_str).unwrap_or_default();
        if PLUGIN_FILE_EXTENSIONS.contains(&ext) {
            validate_plugin_entrypoint_content(path)?;
            return Ok(());
        }
        return Err("Local plugin files must end with .ts, .tsx, .js, or .jsx".to_string());
    }

    if path.is_dir() {
        let source_dir = resolve_plugin_source_dir(path, &path.display().to_string())?;
        validate_plugin_entrypoint(&source_dir, &path.display().to_string())?;
        return Ok(());
    }

    Err(format!("Plugin path does not exist: {}", path.display()))
}

fn plugin_entrypoint_path(path: &Path) -> Option<PathBuf> {
    PLUGIN_ENTRYPOINT_FILES
        .iter()
        .map(|entrypoint| path.join(entrypoint))
        .find(|entrypoint| entrypoint.is_file())
}

fn has_plugin_entrypoint(path: &Path) -> bool {
    plugin_entrypoint_path(path).is_some()
}

fn validate_plugin_entrypoint_content(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("Could not read plugin entrypoint {}: {err}", path.display()))?;
    if content.contains("export default") {
        return Ok(());
    }

    Err(format!(
        "Plugin entrypoint {} must be a Vencord plugin module with a default export. BetterDiscord .plugin.js files are not compatible.",
        path.display()
    ))
}

fn validate_plugin_entrypoint(path: &Path, plugin_name: &str) -> Result<(), String> {
    if let Some(entrypoint) = plugin_entrypoint_path(path) {
        validate_plugin_entrypoint_content(&entrypoint)?;
        return Ok(());
    }

    Err(format!(
        "Plugin source \"{plugin_name}\" is not a Vencord userplugin folder. Expected one of {}.",
        PLUGIN_ENTRYPOINT_FILES.join(", ")
    ))
}

fn collect_plugin_source_dirs(root: &Path, candidates: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(root).map_err(|err| format!("Could not inspect {}: {err}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("Could not inspect {}: {err}", root.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|name| IGNORED_PLUGIN_DISCOVERY_DIRS.contains(&name))
        {
            continue;
        }

        if has_plugin_entrypoint(&path) {
            candidates.push(path);
        } else {
            collect_plugin_source_dirs(&path, candidates)?;
        }
    }
    Ok(())
}

fn resolve_plugin_source_dir(root: &Path, plugin_name: &str) -> Result<PathBuf, String> {
    if has_plugin_entrypoint(root) {
        return Ok(root.to_path_buf());
    }

    let mut candidates = Vec::new();
    collect_plugin_source_dirs(root, &mut candidates)?;
    match candidates.len() {
        0 => Err(format!(
            "Could not find a Vencord plugin in \"{plugin_name}\". Expected one of {} at the repository root or in exactly one subfolder.",
            PLUGIN_ENTRYPOINT_FILES.join(", ")
        )),
        1 => Ok(candidates.remove(0)),
        _ => {
            let shown = candidates
                .iter()
                .take(8)
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            Err(format!(
                "Could not auto-detect one plugin in \"{plugin_name}\" because multiple plugin folders were found:\n{shown}\nAdd a repository that contains exactly one Vencord plugin."
            ))
        }
    }
}

fn copy_dir_all(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let file_type = entry.file_type()?;
        let target = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn reset_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|err| format!("Could not remove {}: {err}", path.display()))?;
    }
    fs::create_dir_all(path).map_err(|err| format!("Could not create {}: {err}", path.display()))
}

fn executable_names(program: &str) -> Vec<String> {
    if cfg!(windows) && Path::new(program).extension().is_none() {
        vec![
            format!("{program}.exe"),
            format!("{program}.cmd"),
            format!("{program}.bat"),
            program.to_string(),
        ]
    } else {
        vec![program.to_string()]
    }
}

fn common_tool_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if cfg!(windows) {
        if let Ok(appdata) = env::var("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("npm"));
        }
        if let Ok(local_appdata) = env::var("LOCALAPPDATA") {
            dirs.push(PathBuf::from(&local_appdata).join("pnpm"));
            dirs.push(PathBuf::from(local_appdata).join("Volta").join("bin"));
        }
        if let Ok(program_files) = env::var("ProgramFiles") {
            dirs.push(PathBuf::from(program_files).join("nodejs"));
        }
        if let Ok(program_files_x86) = env::var("ProgramFiles(x86)") {
            dirs.push(PathBuf::from(program_files_x86).join("nodejs"));
        }
        if let Ok(user_profile) = env::var("USERPROFILE") {
            dirs.push(PathBuf::from(user_profile).join("scoop").join("shims"));
        }
    } else if let Ok(home) = env::var("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local").join("share").join("pnpm"));
        dirs.push(home.join(".cargo").join("bin"));
    }

    dirs
}

fn command_search_dirs() -> Vec<PathBuf> {
    let mut dirs = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    dirs.extend(common_tool_dirs());
    dirs
}

fn command_path_env() -> Option<OsString> {
    env::join_paths(command_search_dirs()).ok()
}

fn resolve_program(program: &str) -> Option<PathBuf> {
    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        return program_path.is_file().then(|| program_path.to_path_buf());
    }

    let names = executable_names(program);
    command_search_dirs()
        .into_iter()
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file())
}

fn run_command(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let resolved_program = resolve_program(program).unwrap_or_else(|| PathBuf::from(program));
    let mut command = Command::new(resolved_program);
    command.args(args);
    if let Some(path) = command_path_env() {
        command.env("PATH", path);
    }
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

    let output = command
        .output()
        .map_err(|err| format!("Failed to run {program}: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut log = String::new();
    log.push_str(&stdout);
    log.push_str(&stderr);

    if output.status.success() {
        Ok(log)
    } else {
        let status = output.status.code().map_or_else(
            || "terminated by signal".to_string(),
            |code| code.to_string(),
        );
        Err(format!(
            "Command failed: {program} {}\nExit status: {status}\n\nstdout:\n{}\n\nstderr:\n{}",
            args.join(" "),
            stdout.trim_end(),
            stderr.trim_end()
        ))
    }
}

fn tool_status(name: &str, version_args: &[&str]) -> ToolStatus {
    match run_command(name, version_args, None) {
        Ok(output) => ToolStatus {
            name: name.to_string(),
            available: true,
            version: output.lines().next().map(str::to_string),
        },
        Err(_) => ToolStatus {
            name: name.to_string(),
            available: false,
            version: None,
        },
    }
}

fn install_pnpm_with_npm(log: &mut String) -> Result<(), String> {
    if !tool_status("npm", &["--version"]).available {
        return Err(format!(
            "Could not install pnpm automatically because Corepack failed and npm is not available on PATH.\n\n\
            Reinstall Node.js with npm/Corepack included, or install pnpm manually, then reopen veskforge.\n\n\
            Recommended manual commands:\n\
            corepack enable pnpm\n\
            corepack prepare pnpm@latest --activate\n\n\
            Installer log:\n{log}"
        ));
    }

    log.push_str("npm install -g pnpm\n");
    match run_command("npm", &["install", "-g", "pnpm"], None) {
        Ok(output) => {
            log.push_str(&output);
            Ok(())
        }
        Err(err) => Err(format!(
            "Could not install pnpm with npm.\n{err}\n\nInstaller log:\n{log}"
        )),
    }
}

#[tauri::command]
fn install_toolchain(app: AppHandle) -> Result<CommandResult, String> {
    let mut log = String::new();
    let git = tool_status("git", &["--version"]);
    let node = tool_status("node", &["--version"]);
    let pnpm = tool_status("pnpm", &["--version"]);

    if git.available && node.available && pnpm.available {
        return Ok(CommandResult {
            ok: true,
            message: "Required build tools are already installed.".to_string(),
            log: format!(
                "git: {}\nnode: {}\npnpm: {}",
                git.version.unwrap_or_else(|| "available".to_string()),
                node.version.unwrap_or_else(|| "available".to_string()),
                pnpm.version.unwrap_or_else(|| "available".to_string()),
            ),
        });
    }

    if !git.available || !node.available {
        let missing = [("git", git.available), ("node", node.available)]
            .into_iter()
            .filter_map(|(name, available)| (!available).then_some(name))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Missing {missing}. Install Git and Node.js first, then run this installer again. veskforge can install pnpm once Node.js is available."
        ));
    }

    log.push_str("Installing pnpm...\n");
    match run_command("corepack", &["enable", "pnpm"], None) {
        Ok(output) => {
            log.push_str("corepack enable pnpm\n");
            log.push_str(&output);
            match run_command("corepack", &["prepare", "pnpm@latest", "--activate"], None) {
                Ok(output) => {
                    log.push_str("corepack prepare pnpm@latest --activate\n");
                    log.push_str(&output);
                }
                Err(err) => {
                    log.push_str(&format!(
                        "Corepack prepare failed, trying npm fallback.\n{err}\n"
                    ));
                    install_pnpm_with_npm(&mut log)?;
                }
            }
        }
        Err(err) => {
            log.push_str(&format!(
                "Corepack unavailable, trying npm fallback.\n{err}\n"
            ));
            install_pnpm_with_npm(&mut log)?;
        }
    }

    let pnpm = tool_status("pnpm", &["--version"]);
    if !pnpm.available {
        return Err(format!(
            "pnpm installation finished, but pnpm is still not available on PATH.\n{log}"
        ));
    }

    let manifest = read_manifest(&app)?;
    let mut result = CommandResult {
        ok: true,
        message: "pnpm installed and build toolchain is ready.".to_string(),
        log: format!(
            "{log}\npnpm: {}\nWorkspace: {}",
            pnpm.version.unwrap_or_else(|| "available".to_string()),
            workspace_dir(&app)?.display(),
        ),
    };
    if manifest.plugins.is_empty() {
        result
            .log
            .push_str("\nNo plugin sources are configured yet.");
    }
    Ok(result)
}

fn git_revision(path: &Path) -> Option<String> {
    run_command("git", &["rev-parse", "--short", "HEAD"], Some(path))
        .ok()
        .map(|output| output.trim().to_string())
        .filter(|output| !output.is_empty())
}

fn vesktop_state_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(path) = env::var("VESKTOP_STATE_FILE") {
        candidates.push(PathBuf::from(path));
    }

    if let Ok(appdata) = env::var("APPDATA") {
        candidates.push(PathBuf::from(&appdata).join("vesktop").join("state.json"));
        candidates.push(PathBuf::from(appdata).join("Vesktop").join("state.json"));
    }

    if let Ok(home) = env::var("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join(".config").join("vesktop").join("state.json"));
        candidates.push(home.join(".config").join("Vesktop").join("state.json"));
        candidates.push(
            home.join(".var")
                .join("app")
                .join("dev.vencord.Vesktop")
                .join("config")
                .join("vesktop")
                .join("state.json"),
        );
    }

    candidates
}

fn find_existing_vesktop_state() -> Option<PathBuf> {
    vesktop_state_candidates()
        .into_iter()
        .find(|path| path.exists())
}

fn materialize_plugins(
    app: &AppHandle,
    manifest: &mut Manifest,
    log: &mut String,
) -> Result<(), String> {
    let userplugins = vencord_dir(app)?.join("src").join("userplugins");
    reset_dir(&userplugins)?;

    for plugin in manifest.plugins.iter_mut().filter(|plugin| plugin.enabled) {
        let target = userplugins.join(&plugin.id);
        match &plugin.source {
            PluginSource::LocalFile { path } => {
                let source_path = Path::new(path);
                validate_local_plugin_path(source_path)?;
                fs::create_dir_all(&target).map_err(|err| {
                    format!("Could not create plugin dir {}: {err}", target.display())
                })?;
                let extension = source_path
                    .extension()
                    .and_then(OsStr::to_str)
                    .unwrap_or("ts");
                fs::copy(path, target.join(format!("index.{extension}")))
                    .map_err(|err| format!("Could not copy plugin file {path}: {err}"))?;
                validate_plugin_entrypoint(&target, &plugin.name)?;
                plugin.installed_path = target.display().to_string();
            }
            PluginSource::LocalFolder { path } => {
                validate_local_plugin_path(Path::new(path))?;
                let source_dir = resolve_plugin_source_dir(Path::new(path), &plugin.name)?;
                copy_dir_all(&source_dir, &target)
                    .map_err(|err| format!("Could not copy plugin folder {path}: {err}"))?;
                validate_plugin_entrypoint(&target, &plugin.name)?;
                plugin.installed_path = target.display().to_string();
            }
            PluginSource::Git { url, reference } => {
                let normalized_url = normalize_github_plugin_url(url)?;
                let store = managed_plugins_dir(app)?.join(&plugin.id);
                if store.exists() {
                    log.push_str(&run_command(
                        "git",
                        &["fetch", "--all", "--tags", "--prune"],
                        Some(&store),
                    )?);
                } else {
                    let parent = store
                        .parent()
                        .ok_or("Could not resolve plugin store parent")?;
                    fs::create_dir_all(parent)
                        .map_err(|err| format!("Could not create plugin store: {err}"))?;
                    log.push_str(&run_command(
                        "git",
                        &["clone", &normalized_url, store.to_str().unwrap_or_default()],
                        None,
                    )?);
                }
                if let Some(reference) = reference
                    .as_ref()
                    .filter(|reference| !reference.trim().is_empty())
                {
                    log.push_str(&run_command("git", &["checkout", reference], Some(&store))?);
                }
                let source_dir = resolve_plugin_source_dir(&store, &plugin.name)?;
                copy_dir_all(&source_dir, &target)
                    .map_err(|err| format!("Could not copy git plugin {normalized_url}: {err}"))?;
                validate_plugin_entrypoint(&target, &plugin.name)?;
                plugin.installed_path = target.display().to_string();
                plugin.last_revision = git_revision(&store);
            }
        }
    }

    Ok(())
}

fn validate_git_plugin_source(
    app: &AppHandle,
    plugin_id: &str,
    url: &str,
    reference: Option<&str>,
    plugin_name: &str,
) -> Result<(), String> {
    let store = managed_plugins_dir(app)?.join(plugin_id);
    if store.exists() {
        run_command(
            "git",
            &["fetch", "--all", "--tags", "--prune"],
            Some(&store),
        )?;
    } else {
        let parent = store
            .parent()
            .ok_or("Could not resolve plugin store parent")?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("Could not create plugin store: {err}"))?;
        run_command(
            "git",
            &["clone", url, store.to_str().unwrap_or_default()],
            None,
        )?;
    }

    if let Some(reference) = reference.filter(|reference| !reference.trim().is_empty()) {
        run_command("git", &["checkout", reference], Some(&store))?;
    }

    let source_dir = resolve_plugin_source_dir(&store, plugin_name)?;
    validate_plugin_entrypoint(&source_dir, plugin_name)
}

fn validate_dist(path: &Path) -> Result<(), String> {
    for file in REQUIRED_DIST_FILES {
        let candidate = path.join(file);
        if !candidate.exists() {
            return Err(format!(
                "Build output is missing required file: {}",
                candidate.display()
            ));
        }
    }
    Ok(())
}

#[tauri::command]
fn get_environment_status(app: AppHandle) -> Result<EnvironmentStatus, String> {
    let manifest = read_manifest(&app)?;
    let app_data = app_data_dir(&app)?;
    let workspace = workspace_dir(&app)?;
    let vencord = vencord_dir(&app)?;
    let dist = dist_dir(&app)?;
    let candidates = vesktop_state_candidates();

    Ok(EnvironmentStatus {
        app_data_dir: app_data.display().to_string(),
        workspace_dir: workspace.display().to_string(),
        vencord_dir: vencord.display().to_string(),
        dist_dir: dist.display().to_string(),
        vesktop_state_candidates: candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        selected_vesktop_state: find_existing_vesktop_state()
            .map(|path| path.display().to_string()),
        tools: vec![
            tool_status("git", &["--version"]),
            tool_status("node", &["--version"]),
            tool_status("pnpm", &["--version"]),
        ],
        manifest,
    })
}

#[tauri::command]
fn add_plugin(app: AppHandle, request: AddPluginRequest) -> Result<Manifest, String> {
    let source = match request.source {
        PluginSource::LocalFile { path } => {
            validate_local_plugin_path(Path::new(&path))?;
            PluginSource::LocalFile { path }
        }
        PluginSource::LocalFolder { path } => {
            validate_local_plugin_path(Path::new(&path))?;
            PluginSource::LocalFolder { path }
        }
        PluginSource::Git { url, reference } => PluginSource::Git {
            url: normalize_github_plugin_url(&url)?,
            reference,
        },
    };

    let mut manifest = read_manifest(&app)?;
    let name = plugin_name_from_source(&source, request.name);
    let mut id = sanitize_id(&name);
    let base_id = id.clone();
    let mut suffix = 2;
    while manifest.plugins.iter().any(|plugin| plugin.id == id) {
        id = format!("{base_id}-{suffix}");
        suffix += 1;
    }

    if let PluginSource::Git { url, reference } = &source {
        validate_git_plugin_source(&app, &id, url, reference.as_deref(), &name)?;
    }

    manifest.plugins.push(PluginRecord {
        id,
        name,
        source,
        enabled: true,
        installed_path: String::new(),
        git_ref: None,
        last_revision: None,
    });
    write_manifest(&app, &manifest)?;
    Ok(manifest)
}

#[tauri::command]
fn remove_plugin(app: AppHandle, plugin_id: String) -> Result<Manifest, String> {
    let mut manifest = read_manifest(&app)?;
    manifest.plugins.retain(|plugin| plugin.id != plugin_id);
    write_manifest(&app, &manifest)?;
    Ok(manifest)
}

#[tauri::command]
fn set_plugin_enabled(
    app: AppHandle,
    plugin_id: String,
    enabled: bool,
) -> Result<Manifest, String> {
    let mut manifest = read_manifest(&app)?;
    let plugin = manifest
        .plugins
        .iter_mut()
        .find(|plugin| plugin.id == plugin_id)
        .ok_or_else(|| format!("Unknown plugin: {plugin_id}"))?;
    plugin.enabled = enabled;
    write_manifest(&app, &manifest)?;
    Ok(manifest)
}

#[tauri::command]
fn set_update_policy(app: AppHandle, request: SetUpdatePolicyRequest) -> Result<Manifest, String> {
    if !matches!(request.mode.as_str(), "manual" | "auto") {
        return Err("Update policy must be manual or auto".to_string());
    }
    let mut manifest = read_manifest(&app)?;
    manifest.update_policy.mode = request.mode;
    write_manifest(&app, &manifest)?;
    Ok(manifest)
}

#[tauri::command]
fn check_updates(app: AppHandle) -> Result<CommandResult, String> {
    let vencord = vencord_dir(&app)?;
    if !vencord.exists() {
        return Ok(CommandResult {
            ok: true,
            message: "Vencord has not been cloned yet.".to_string(),
            log: "Run Build to clone Vencord first.".to_string(),
        });
    }

    let log = run_command("git", &["fetch", "--tags", "--prune"], Some(&vencord))?;
    let local = run_command("git", &["rev-parse", "--short", "HEAD"], Some(&vencord))?;
    let remote = run_command(
        "git",
        &["rev-parse", "--short", "origin/main"],
        Some(&vencord),
    )?;
    let up_to_date = local.trim() == remote.trim();
    Ok(CommandResult {
        ok: up_to_date,
        message: if up_to_date {
            "Managed Vencord checkout is up to date.".to_string()
        } else {
            format!("Update available: {} -> {}", local.trim(), remote.trim())
        },
        log,
    })
}

#[tauri::command]
fn build_vencord(app: AppHandle) -> Result<CommandResult, String> {
    let mut manifest = read_manifest(&app)?;
    let vencord = vencord_dir(&app)?;
    let mut log = String::new();

    if vencord.exists() {
        log.push_str("Updating managed Vencord checkout...\n");
        log.push_str(&run_command(
            "git",
            &["fetch", "--tags", "--prune"],
            Some(&vencord),
        )?);
        log.push_str(&run_command("git", &["checkout", "main"], Some(&vencord))?);
        log.push_str(&run_command("git", &["pull", "--ff-only"], Some(&vencord))?);
    } else {
        log.push_str("Cloning Vencord...\n");
        let workspace = workspace_dir(&app)?;
        log.push_str(&run_command(
            "git",
            &["clone", VENCORD_REPO, "Vencord"],
            Some(&workspace),
        )?);
    }

    materialize_plugins(&app, &mut manifest, &mut log)?;
    log.push_str("Installing Vencord dependencies...\n");
    log.push_str(&run_command(
        "pnpm",
        &["install", "--frozen-lockfile"],
        Some(&vencord),
    )?);
    log.push_str("Building Vencord desktop artifacts...\n");
    log.push_str(&run_command("pnpm", &["build"], Some(&vencord))?);

    let dist = dist_dir(&app)?;
    validate_dist(&dist)?;
    manifest.last_successful_build = Some(BuildMetadata {
        built_at: now_stamp(),
        dist_path: dist.display().to_string(),
        vencord_revision: git_revision(&vencord),
    });
    write_manifest(&app, &manifest)?;

    Ok(CommandResult {
        ok: true,
        message: "Vencord build completed and dist was validated.".to_string(),
        log,
    })
}

#[tauri::command]
fn apply_to_vesktop(app: AppHandle, request: ApplyRequest) -> Result<CommandResult, String> {
    let dist = dist_dir(&app)?;
    validate_dist(&dist)?;

    let state_path = request
        .state_path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .or_else(find_existing_vesktop_state)
        .ok_or_else(|| "Could not locate Vesktop state.json. Set VESKTOP_STATE_FILE or paste the path in the UI.".to_string())?;

    let mut state: Value = if state_path.exists() {
        let content = fs::read_to_string(&state_path)
            .map_err(|err| format!("Could not read {}: {err}", state_path.display()))?;
        serde_json::from_str(&content)
            .map_err(|err| format!("Could not parse {}: {err}", state_path.display()))?
    } else {
        json!({})
    };

    if !state.is_object() {
        return Err("Vesktop state.json must contain a JSON object".to_string());
    }
    state["vencordDir"] = Value::String(dist.display().to_string());

    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Could not create {}: {err}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(&state)
        .map_err(|err| format!("Could not serialize Vesktop state: {err}"))?;
    fs::write(&state_path, content)
        .map_err(|err| format!("Could not write {}: {err}", state_path.display()))?;

    Ok(CommandResult {
        ok: true,
        message:
            "Vesktop now points at the veskforge Vencord build. Fully restart Vesktop to apply it."
                .to_string(),
        log: format!(
            "Updated {}\nvencordDir={}",
            state_path.display(),
            dist.display()
        ),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_environment_status,
            add_plugin,
            remove_plugin,
            set_plugin_enabled,
            set_update_policy,
            install_toolchain,
            check_updates,
            build_vencord,
            apply_to_vesktop
        ])
        .run(tauri::generate_context!())
        .expect("error while running veskforge");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sanitize_id_keeps_cli_safe_names() {
        assert_eq!(sanitize_id("My Cool Plugin"), "my-cool-plugin");
        assert_eq!(sanitize_id("Vencord++ Tools"), "vencord-tools");
    }

    #[test]
    fn local_plugin_file_validation_accepts_supported_source_extensions() {
        let dir = env::temp_dir().join(format!("veskforge-test-{}", now_stamp()));
        fs::create_dir_all(&dir).unwrap();

        let ts = dir.join("plugin.ts");
        let tsx = dir.join("plugin.tsx");
        let js = dir.join("plugin.js");
        let jsx = dir.join("plugin.jsx");
        let plugin_js = dir.join("plugin.plugin.js");
        let css = dir.join("plugin.css");
        for path in [&ts, &tsx, &js, &jsx, &plugin_js] {
            let mut file = fs::File::create(path).unwrap();
            writeln!(file, "export default {{}};").unwrap();
        }
        fs::write(&css, "body {}").unwrap();

        assert!(validate_local_plugin_path(&ts).is_ok());
        assert!(validate_local_plugin_path(&tsx).is_ok());
        assert!(validate_local_plugin_path(&js).is_ok());
        assert!(validate_local_plugin_path(&jsx).is_ok());
        assert!(validate_local_plugin_path(&plugin_js).is_ok());
        assert!(validate_local_plugin_path(&css).is_err());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn local_plugin_file_rejects_commonjs_plugin_files() {
        let dir = env::temp_dir().join(format!("veskforge-commonjs-test-{}", now_stamp()));
        fs::create_dir_all(&dir).unwrap();

        let betterdiscord = dir.join("plugin.plugin.js");
        fs::write(&betterdiscord, "module.exports = class Plugin {};").unwrap();
        assert!(validate_local_plugin_path(&betterdiscord).is_err());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn local_plugin_folder_requires_index_entrypoint() {
        let dir = env::temp_dir().join(format!("veskforge-folder-test-{}", now_stamp()));
        fs::create_dir_all(&dir).unwrap();
        assert!(validate_local_plugin_path(&dir).is_err());

        fs::write(dir.join("index.jsx"), "export default {};").unwrap();
        assert!(validate_local_plugin_path(&dir).is_ok());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn github_plugin_urls_are_repository_roots() {
        assert_eq!(
            normalize_github_plugin_url("https://github.com/Microck/discord-gfm-tables").unwrap(),
            "https://github.com/Microck/discord-gfm-tables.git"
        );
        assert_eq!(
            normalize_github_plugin_url("https://github.com/Microck/discord-gfm-tables.git")
                .unwrap(),
            "https://github.com/Microck/discord-gfm-tables.git"
        );

        assert!(normalize_github_plugin_url(
            "https://github.com/Microck/discord-gfm-tables/blob/main/index.ts"
        )
        .is_err());
        assert!(normalize_github_plugin_url(
            "https://raw.githubusercontent.com/Microck/discord-gfm-tables/main/index.ts"
        )
        .is_err());
        assert!(
            normalize_github_plugin_url("git@github.com:Microck/discord-gfm-tables.git").is_err()
        );
    }

    #[test]
    fn materialized_plugin_requires_index_entrypoint() {
        let dir = env::temp_dir().join(format!("veskforge-entrypoint-test-{}", now_stamp()));
        fs::create_dir_all(&dir).unwrap();
        assert!(validate_plugin_entrypoint(&dir, "Missing Entrypoint").is_err());

        fs::write(dir.join("index.js"), "export default {};").unwrap();
        assert!(validate_plugin_entrypoint(&dir, "Missing Entrypoint").is_ok());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn plugin_source_dir_auto_detects_one_nested_plugin() {
        let dir = env::temp_dir().join(format!("veskforge-detect-test-{}", now_stamp()));
        let plugin = dir.join("packages").join("the-plugin");
        fs::create_dir_all(&plugin).unwrap();
        fs::write(plugin.join("index.tsx"), "export default {};").unwrap();

        assert_eq!(
            resolve_plugin_source_dir(&dir, "Nested Plugin").unwrap(),
            plugin
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn plugin_source_dir_rejects_ambiguous_repositories() {
        let dir = env::temp_dir().join(format!("veskforge-ambiguous-test-{}", now_stamp()));
        let first = dir.join("first");
        let second = dir.join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        fs::write(first.join("index.ts"), "export default {};").unwrap();
        fs::write(second.join("index.tsx"), "export default {};").unwrap();

        assert!(resolve_plugin_source_dir(&dir, "Ambiguous").is_err());

        fs::remove_dir_all(dir).unwrap();
    }
}
