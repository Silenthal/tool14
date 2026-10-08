// install finding code from install.rs in ironworks
use anyhow::{Context, Result, anyhow};
use config::{Config, File as ConfigFile};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

static SETTINGS: OnceLock<Settings> = OnceLock::new();

const TRY_PATHS: &[&str] = &[
    r"C:\SquareEnix\FINAL FANTASY XIV - A Realm Reborn",
    r"C:\Program Files (x86)\Steam\steamapps\common\FINAL FANTASY XIV Online",
    r"C:\Program Files (x86)\Steam\steamapps\common\FINAL FANTASY XIV - A Realm Reborn",
    r"C:\Program Files (x86)\FINAL FANTASY XIV - A Realm Reborn",
    r"C:\Program Files (x86)\SquareEnix\FINAL FANTASY XIV - A Realm Reborn",
    r"C:\Program Files (x86)\上海数龙科技有限公司\最终幻想XIV",
    r"C:\WeGameApps\rail_apps\ffxiv(2000340)",
];

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_path")]
    pub game_path: PathBuf,

    #[serde(default = "default_folder")]
    pub schema_folder: PathBuf,

    #[serde(default = "default_pull")]
    pub schema_pull_on_launch: Option<bool>,

    #[serde(default = "default_repo")]
    pub schema_remote_repo: Option<String>,

    #[serde(default = "default_branch")]
    pub schema_remote_branch: Option<String>,
}

fn default_path() -> PathBuf {
    PathBuf::from(
        r"C:\Program Files (x86)\Steam\steamapps\common\FINAL FANTASY XIV - A Realm Reborn",
    )
}

fn default_folder() -> PathBuf {
    PathBuf::from("schemas")
}

fn default_repo() -> Option<String> {
    Some("https://github.com/xivdev/EXDSchema.git".to_string())
}

fn default_branch() -> Option<String> {
    Some("latest".to_string())
}

fn default_pull() -> Option<bool> {
    Some(true)
}

impl Settings {
    pub fn global() -> &'static Settings {
        SETTINGS
            .get()
            .expect("Settings must be initialized before use")
    }

    pub fn init_resources(&mut self, path: Option<&PathBuf>) -> Result<()> {
        if let Some(p) = path && p.exists(){
            self.game_path = p.to_path_buf();
        }
        else if !self.game_path.exists() {
            println!("Check for {} fail", self.game_path.display());
            self.game_path =
                find_install().ok_or_else(|| anyhow!("Unable to find FFXIV install path."))?;
        }
        let _ = self.get_game_ver()?;
        if let Some(true) = self.schema_pull_on_launch {
            // Check for existence of dir
            let Some(branch) = self.schema_remote_branch.clone().or(default_branch()) else {
                println!("Unable to get remote schema branch name, continuing");
                return Ok(());
            };
            let Some(remote) = self.schema_remote_repo.clone().or(default_repo()) else {
                println!("Unable to get remote schema path, continuing");
                return Ok(());
            };
            sync_repo(&self.schema_folder, &remote, &branch)?;
        }
        Ok(())
    }

    pub fn get_game_ver(&self) -> Result<String> {
        let ver_path = self.game_path.join("game").join("ffxivgame.ver");
        let mut ver_file = File::open(ver_path).context("Unable to open version file. Check that the given FFXIV install path is correct.")?;
        let mut ver_str = String::new();
        ver_file
            .read_to_string(&mut ver_str)
            .context("Unable to read from version file. Check that the given FFXIV install path is correct.")?;
        Ok(ver_str)
    }
}

pub fn load(path: Option<&PathBuf>) -> Result<()> {
    let config_path = get_config_path()?;
    let mut settings: Settings = Config::builder()
        .add_source(ConfigFile::from(config_path.as_path()).required(false))
        .build()
        .context("Unable to build config file")?
        .try_deserialize()
        .context("Unable to deserialize config")?;
    settings.init_resources(path)?;
    save(&settings)?;
    SETTINGS
        .set(settings)
        .map_err(|_| anyhow!("Settings are already initialized."))?;
    Ok(())
}

pub fn save(settings: &Settings) -> Result<()> {
    let settings_path = get_config_path()?;
    let settings_str =
        serde_json::to_string_pretty(&settings).with_context(|| "Unable to serialize settings")?;
    fs::write(settings_path, settings_str).with_context(|| "Unable to write to settings file")?;
    Ok(())
}

pub fn get_config_path() -> Result<PathBuf> {
    let mut config_path;
    if cfg!(debug_assertions) {
        config_path = PathBuf::from("config.json");
    } else {
        config_path = env::current_exe()?.to_path_buf();
        config_path.pop();
        config_path.push("config.json");
    }
    Ok(config_path)
}

pub fn get_cache_path() -> Result<PathBuf> {
    let mut config_path;
    if cfg!(debug_assertions) {
        config_path = PathBuf::from("cache.sqlite3");
    } else {
        config_path = env::current_exe()?.to_path_buf();
        config_path.pop();
        config_path.push("cache.sqlite3");
    }
    Ok(config_path)
}

pub fn find_install() -> Option<PathBuf> {
    for path_str in TRY_PATHS {
        let win_path = PathBuf::from(path_str);
        if win_path.exists() {
            return Some(win_path);
        }
        let mut wsl_path = PathBuf::from("/mnt/c");
        let win_path_parts = win_path
            .components()
            .filter(|c| !matches!(c, std::path::Component::Prefix(_)));
        for part in win_path_parts {
            if let std::path::Component::Normal(p) = part {
                wsl_path.push(p);
            }
        }
        if wsl_path.exists() {
            return Some(wsl_path);
        }
    }
    None
}

fn sync_repo(path: &Path, url: &str, branch: &str) -> Result<()> {
    if let Err(_) = run_git_command(None, &["--version"]) {
        return Err(anyhow!("Git is not detected. Unable to sync schemas."));
    }

    if !path.exists() {
        return clone_repo(path, url, branch);
    }

    {
        let is_repo_dir_git;
        match run_git_command(Some(path), &["rev-parse", "--is-inside-work-tree"]) {
            Ok(res) => is_repo_dir_git = res == "true",
            Err(e) => {
                if !path.join(".git").exists() {
                    is_repo_dir_git = false;
                } else {
                    return Err(e);
                }
            }
        }

        let is_repo_dir_empty = fs::read_dir(path)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(true);
        if !is_repo_dir_git {
            if !is_repo_dir_empty {
                return Err(anyhow!(
                    "The directory '{}' isn't a git repository and contains files.\n\
                    Empty the directory, or set the property 'schema_pull_on_launch' in the config file to false to skip checking.",
                    path.display()
                ));
            } else {
                fs::remove_dir_all(path).with_context(|| {
                    format!("Failed to remove existing directory before cloning from remote repo")
                })?;
                return clone_repo(path, url, branch);
            }
        }
    }

    // Folder exists, and is repo - check for updates
    // This one can fail, since remote hash also checks via internet
    match run_git_command(Some(path), &["fetch", "origin", branch]) {
        Ok(_) => {
            let remote_branch_str = format!("origin/{branch}");
            let local_hash = run_git_command(Some(path), &["rev-parse", "HEAD"])?;
            let remote_hash = run_git_command(Some(path), &["rev-parse", &remote_branch_str])?;

            if local_hash != remote_hash {
                println!("Updating schema from remote");
                run_git_command(Some(path), &["reset", "--hard", &remote_branch_str])?;
            }
        }
        Err(_) => println!("Unable to fetch remote schema, continuing with local"),
    }

    Ok(())
}

fn clone_repo(path: &Path, url: &str, branch: &str) -> Result<()> {
    fs::create_dir_all(path).with_context(|| "Unable to create directory before cloning")?;
    run_git_command(Some(path), &["clone", "-b", branch, url, "."])?;
    Ok(())
}

fn run_git_command(path: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut command = Command::new("git");
    if let Some(p) = path {
        command.arg("-C").arg(p);
    }
    command.args(args);

    let output = command
        .output()
        .with_context(|| "Failed to execute git command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let exit_code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        return Err(anyhow!(
            "Git command failed with error code {}: {}",
            exit_code,
            if stderr.is_empty() { stdout } else { stderr }
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
