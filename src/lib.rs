use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
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
    /// Returns an error if the file cannot be written or permissions cannot be set.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy();

        let content = self.generate_content();
        fs::write(path_ref, &content)
            .with_context(|| format!("failed to write state file to {path_str}"))?;

        // Fix permissions - set to 644 (rw-r--r--)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path_ref)
                .with_context(|| format!("failed to get metadata for {path_str}"))?
                .permissions();
            perms.set_mode(0o644);
            fs::set_permissions(path_ref, perms)
                .with_context(|| format!("failed to set permissions for {path_str}"))?;
        }

        Ok(())
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
