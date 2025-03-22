use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

// Custom error type for better error handling
#[derive(Debug)]
pub enum ModuleError {
    IoError(std::io::Error),
    ParseError(String),
    ModuleNotFound(String),
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleError::IoError(err) => write!(f, "IO error: {err}"),
            ModuleError::ParseError(msg) => write!(f, "Parse error: {msg}"),
            ModuleError::ModuleNotFound(name) => write!(f, "Module not found: {name}"),
        }
    }
}

impl Error for ModuleError {}

impl From<std::io::Error> for ModuleError {
    fn from(err: std::io::Error) -> Self {
        ModuleError::IoError(err)
    }
}

// Module data structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleRegistry {
    pub modules: Vec<Module>,
    #[serde(skip)]
    module_map: Option<HashMap<String, String>>, // name -> path
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Module {
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleStatus {
    pub name: String,
    pub path: String,
    pub enabled: bool,
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
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ModuleError> {
        let json_content = fs::read_to_string(path)?;
        let mut registry: ModuleRegistry = serde_json::from_str(&json_content)
            .map_err(|e| ModuleError::ParseError(e.to_string()))?;

        // Initialize lookup map for efficiency
        registry.init_lookup();
        Ok(registry)
    }

    // Initialize the lookup map for efficient path retrieval
    pub fn init_lookup(&mut self) {
        let mut map = HashMap::new();
        for module in &self.modules {
            map.insert(module.name.clone(), module.path.clone());
        }
        self.module_map = Some(map);
    }

    // Get module path efficiently using HashMap
    #[must_use]
    pub fn get_module_path(&self, module_name: &str) -> Option<String> {
        if let Some(map) = &self.module_map {
            map.get(module_name).cloned()
        } else {
            // Fallback to linear search if map not initialized
            for module in &self.modules {
                if module.name == module_name {
                    return Some(module.path.clone());
                }
            }
            None
        }
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
    pub fn get_lookup_map(&self) -> Option<&HashMap<String, String>> {
        self.module_map.as_ref()
    }
}

// ModuleFile manages parsing and generating the modules file
pub struct ModuleFile {
    pub active_modules: Vec<String>,
    content: Option<String>,
}

impl ModuleFile {
    /// Create a new `ModuleFile` by reading from path
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ModuleError> {
        if !path.as_ref().exists() {
            return Ok(Self {
                active_modules: Vec::new(),
                content: None,
            });
        }

        let content = fs::read_to_string(path)?;
        let active_modules = Self::parse_active_modules(&content);

        Ok(Self {
            active_modules,
            content: Some(content),
        })
    }

    // Create an empty ModuleFile
    #[must_use]
    pub fn empty() -> Self {
        Self {
            active_modules: Vec::new(),
            content: None,
        }
    }

    // Parse module names from file content (made public for testing)
    #[must_use]
    pub fn parse_active_modules(content: &str) -> Vec<String> {
        let mut active_modules = Vec::new();

        // Extract module names from nix store path lines
        for line in content.lines() {
            let line = line.trim();
            if line.contains("/nix/store/") && line.contains("-source/") {
                if let Some(comment_pos) = line.find('#') {
                    let comment_part = &line[comment_pos + 1..];
                    let module_name = comment_part.trim();

                    if !module_name.is_empty() {
                        active_modules.push(module_name.to_string());
                    }
                }
            }
        }

        active_modules
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

    // Generate file content with the current active modules
    #[must_use]
    pub fn generate_content(&self, registry: &ModuleRegistry) -> String {
        let module_paths: Vec<(String, String)> = self
            .active_modules
            .iter()
            .filter_map(|module| {
                registry
                    .get_module_path(module)
                    .map(|path| (module.clone(), path))
            })
            .collect();

        Self::generate_file_content(&self.active_modules, &module_paths)
    }

    // Static method to generate file content
    fn generate_file_content(modules: &[String], module_paths: &[(String, String)]) -> String {
        let mut content = String::from("# This file is generated by runtime-module script\n");
        content.push_str("{ ... }:\n");
        content.push_str("{\n");

        if modules.is_empty() {
            content.push_str("  # No active modules\n");
        } else {
            content.push_str("  imports = [\n");

            // Add each module path
            for module in modules {
                // Find path for this module
                if let Some((_, path)) = module_paths.iter().find(|(name, _)| name == module) {
                    content.push_str(&format!("    \"{path}\" # {module}\n"));
                }
            }

            content.push_str("  ];\n");
        }

        content.push_str("}\n");

        content
    }

    /// Save the module file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written or permissions cannot be set.
    pub fn save<P: AsRef<Path>>(
        &self,
        path: P,
        registry: &ModuleRegistry,
    ) -> Result<(), ModuleError> {
        let content = self.generate_content(registry);
        fs::write(&path, &content)?;

        // Fix permissions - set to 644 (rw-r--r--)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    // Access content (for silencing 'never read' warning)
    #[must_use]
    pub fn get_content(&self) -> Option<&String> {
        self.content.as_ref()
    }
}
