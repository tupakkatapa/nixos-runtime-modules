use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

// A module's state compares what was requested (the state file) with what the
// running system was built with (the marker the runtime flake writes to /etc)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub enum ModuleState {
    Enabled,
    #[default]
    Disabled,
    Uncertain,
}

impl ModuleState {
    // `applied` is None when the running system does not say what it was built with
    #[must_use]
    pub fn of(requested: bool, applied: Option<bool>) -> Self {
        match (requested, applied) {
            (true, Some(true)) => Self::Enabled,
            (false, Some(false)) => Self::Disabled,
            _ => Self::Uncertain,
        }
    }
}

// Module data structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleRegistry {
    pub modules: Vec<Module>,
    #[serde(skip)]
    module_map: Option<HashMap<String, usize>>, // name -> index in modules vector
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub post_enable: Option<String>,
    #[serde(default)]
    pub post_disable: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModuleStatus {
    pub name: String,
    #[serde(default)]
    pub path: String,
    pub state: ModuleState,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub post_enable: Option<String>,
    #[serde(default)]
    pub post_disable: Option<String>,
}

impl ModuleRegistry {
    // Constructor for creating a new registry
    #[must_use]
    pub fn new(modules: Vec<Module>) -> Self {
        Self {
            modules,
            module_map: None,
        }
    }

    /// Load registry from file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or if it contains invalid JSON.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy();
        let json_content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read registry from {path_str}"))?;

        let mut registry: ModuleRegistry = serde_json::from_str(&json_content)
            .with_context(|| format!("failed to parse JSON from {path_str}"))?;

        // Initialize lookup map for efficiency
        registry.init_lookup();
        Ok(registry)
    }

    // Initialize the lookup map for efficient module retrieval
    pub fn init_lookup(&mut self) {
        let mut map = HashMap::new();
        for (index, module) in self.modules.iter().enumerate() {
            map.insert(module.name.clone(), index);
        }
        self.module_map = Some(map);
    }

    // Check if all modules exist in the registry
    #[must_use]
    pub fn verify_modules_exist(&self, modules: &[String]) -> bool {
        if let Some(map) = &self.module_map {
            modules.iter().all(|module| map.contains_key(module))
        } else {
            let available_modules: HashSet<_> = self.modules.iter().map(|m| &m.name).collect();
            modules
                .iter()
                .all(|module| available_modules.contains(module))
        }
    }

    // Method for checking if the lookup map is initialized (for testing)
    #[must_use]
    pub fn has_lookup_map(&self) -> bool {
        self.module_map.is_some()
    }

    // Getter for lookup map (for testing)
    #[must_use]
    pub fn get_lookup_map(&self) -> Option<&HashMap<String, usize>> {
        self.module_map.as_ref()
    }
}

// State file format for enabled modules
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct StateFile {
    #[serde(default)]
    pub enabled: Vec<String>,
}

// ModuleFile manages the state of enabled modules
pub struct ModuleFile {
    pub active_modules: Vec<String>,
}

impl ModuleFile {
    /// Create a new `ModuleFile` by reading from JSON state file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();

        if !path_ref.exists() {
            return Ok(Self {
                active_modules: Vec::new(),
            });
        }

        let path_str = path_ref.to_string_lossy();
        let content = fs::read_to_string(path_ref)
            .with_context(|| format!("failed to read state file from {path_str}"))?;

        let state: StateFile = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse JSON from {path_str}"))?;

        Ok(Self {
            active_modules: state.enabled,
        })
    }

    // Create an empty ModuleFile
    #[must_use]
    pub fn empty() -> Self {
        Self {
            active_modules: Vec::new(),
        }
    }

    // Parse module names from JSON content
    #[must_use]
    pub fn parse_active_modules(content: &str) -> Vec<String> {
        serde_json::from_str::<StateFile>(content)
            .map(|s| s.enabled)
            .unwrap_or_default()
    }

    // Check if a module is enabled
    #[must_use]
    pub fn is_module_enabled(&self, module_name: &str) -> bool {
        self.active_modules.iter().any(|name| name == module_name)
    }

    // Enable modules and return if changes were made
    pub fn enable_modules(&mut self, modules: &[String]) -> bool {
        let mut changes = false;

        for module in modules {
            if !self.is_module_enabled(module) {
                self.active_modules.push(module.clone());
                changes = true;
            }
        }

        changes
    }

    // Disable modules and return if changes were made
    pub fn disable_modules(&mut self, modules: &[String]) -> bool {
        let disable_set: HashSet<_> = modules.iter().collect();
        let original_len = self.active_modules.len();

        self.active_modules
            .retain(|module| !disable_set.contains(module));

        original_len != self.active_modules.len()
    }

    // Generate JSON content with enabled modules
    #[must_use]
    pub fn generate_content(&self) -> String {
        let state = StateFile {
            enabled: self.active_modules.clone(),
        };
        serde_json::to_string_pretty(&state).unwrap_or_else(|_| r#"{"enabled":[]}"#.to_string())
    }

    /// Save the state file as JSON
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        write_atomic(path, &self.generate_content())
    }
}

/// Read the modules the running system was built with. A missing marker means
/// the base system; `None` means a marker without a module list.
///
/// # Errors
///
/// Returns an error if the marker exists but cannot be read.
pub fn read_applied<P: AsRef<Path>>(path: P) -> Result<Option<Vec<String>>> {
    let path_ref = path.as_ref();
    match fs::read_to_string(path_ref) {
        Ok(content) => Ok(serde_json::from_str::<StateFile>(&content)
            .ok()
            .map(|state| state.enabled)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Some(Vec::new())),
        Err(e) => Err(e).with_context(|| format!("failed to read {}", path_ref.to_string_lossy())),
    }
}

// Whether the running system was built with exactly the requested modules
#[must_use]
pub fn in_sync(requested: &[String], applied: Option<&[String]>) -> bool {
    applied.is_some_and(|applied| {
        requested.iter().collect::<HashSet<_>>() == applied.iter().collect::<HashSet<_>>()
    })
}

/// Replace a file through a rename, so readers see the old or the new content
/// and never a partial write.
///
/// # Errors
///
/// Returns an error if the temporary file cannot be written or renamed.
pub fn write_atomic<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
    let path_ref = path.as_ref();
    let path_str = path_ref.to_string_lossy();
    let tmp = path_ref.with_file_name(format!(
        ".{}.tmp",
        path_ref.file_name().unwrap_or_default().to_string_lossy()
    ));

    let mut file = File::create(&tmp)
        .with_context(|| format!("failed to create temporary file for {path_str}"))?;
    file.write_all(content.as_bytes())
        .and_then(|()| file.set_permissions(fs::Permissions::from_mode(0o644)))
        .and_then(|()| file.sync_all())
        .with_context(|| format!("failed to write {path_str}"))?;
    fs::rename(&tmp, path_ref).with_context(|| format!("failed to replace {path_str}"))?;

    Ok(())
}

// Exclusive lock serialising operations that change the system; released on drop
pub struct OperationLock {
    _file: File,
}

impl OperationLock {
    /// Take the lock without waiting; `None` when another holder has it.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock file cannot be opened or locked.
    pub fn try_acquire<P: AsRef<Path>>(path: P) -> Result<Option<Self>> {
        let file = Self::open(path.as_ref())?;
        if Self::flock(&file, libc::LOCK_EX | libc::LOCK_NB)? {
            Ok(Some(Self { _file: file }))
        } else {
            Ok(None)
        }
    }

    /// Take the lock, calling `on_wait` once if another holder has it first.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock file cannot be opened or locked.
    pub fn acquire<P: AsRef<Path>>(path: P, on_wait: impl FnOnce()) -> Result<Self> {
        if let Some(lock) = Self::try_acquire(&path)? {
            return Ok(lock);
        }
        on_wait();
        let file = Self::open(path.as_ref())?;
        Self::flock(&file, libc::LOCK_EX)?;
        Ok(Self { _file: file })
    }

    fn open(path: &Path) -> Result<File> {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .with_context(|| format!("failed to open lock file {}", path.to_string_lossy()))
    }

    // Ok(false) when non-blocking and already held
    fn flock(file: &File, operation: libc::c_int) -> Result<bool> {
        if unsafe { libc::flock(file.as_raw_fd(), operation) } == 0 {
            return Ok(true);
        }
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::WouldBlock {
            Ok(false)
        } else {
            Err(err).context("failed to lock")
        }
    }
}
