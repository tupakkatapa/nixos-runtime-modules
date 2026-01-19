use crate::system::apply_configuration;
use anyhow::{Context, Result};
use runtime_modules::{ModuleFile, ModuleRegistry, ModuleState, ModuleStatus};

// Constants
const MODULES_JSON: &str = "/run/runtime-modules/modules.json";
const MODULES_FILE: &str = "/run/runtime-modules/runtime-modules.nix";

// ModuleManager handles the business logic
pub struct ModuleManager {
    registry: ModuleRegistry,
    module_file: ModuleFile,
}

impl ModuleManager {
    // Initialize the manager by loading registry and module file
    pub fn new() -> Result<Self> {
        let registry =
            ModuleRegistry::from_file(MODULES_JSON).context("failed to load module registry")?;
        let module_file =
            ModuleFile::from_file(MODULES_FILE).context("failed to load module file")?;

        // Update the registry states based on active modules
        let mut manager = Self {
            registry,
            module_file,
        };

        // Sync state with module file at initialization
        manager.sync_registry_with_module_file();

        Ok(manager)
    }

    // Sync registry state with active modules in module file
    fn sync_registry_with_module_file(&mut self) {
        // Make sure modules in the module file are marked as Enabled in the registry
        for module in &self.module_file.active_modules {
            if self.registry.get_state(module) != ModuleState::Uncertain {
                self.registry.set_state(module, ModuleState::Enabled);
            }
        }
    }

    // Helper method to get the effective state of a module
    fn get_effective_state(&self, module: &str) -> ModuleState {
        let is_in_config = self.module_file.is_module_enabled(module);
        let state = self.registry.get_state(module);

        if state == ModuleState::Uncertain {
            ModuleState::Uncertain
        } else if is_in_config {
            ModuleState::Enabled
        } else {
            ModuleState::Disabled
        }
    }

    // Get status for specific modules
    pub fn get_status(&self, modules: &[String]) -> Vec<ModuleStatus> {
        modules
            .iter()
            .map(|module| {
                let state = self.get_effective_state(module);

                // Find module in registry for details
                let module_index = self
                    .registry
                    .get_lookup_map()
                    .and_then(|map| map.get(module))
                    .copied();

                if let Some(index) = module_index {
                    let registry_module = &self.registry.modules[index];
                    ModuleStatus {
                        name: module.clone(),
                        path: registry_module.path.clone(),
                        state,
                        desc: registry_module.desc.clone(),
                    }
                } else {
                    // Fallback if module not found
                    ModuleStatus {
                        name: module.clone(),
                        path: String::new(),
                        state,
                        desc: String::new(),
                    }
                }
            })
            .collect()
    }

    // Get status for all modules
    pub fn get_all_status(&self) -> Vec<ModuleStatus> {
        self.registry
            .modules
            .iter()
            .map(|module| {
                let name = &module.name;
                let state = self.get_effective_state(name);

                ModuleStatus {
                    name: name.clone(),
                    path: module.path.clone(),
                    state,
                    desc: module.desc.clone(),
                }
            })
            .collect()
    }

    // Apply changes and persist state
    fn apply_changes(&mut self, _force: bool, action_msg: &str) -> Result<()> {
        // Save the module file
        self.module_file
            .save(MODULES_FILE, &self.registry)
            .with_context(|| format!("failed to save module file after {action_msg}"))?;
        println!("generated modules file at '{MODULES_FILE}'");

        // Apply configuration
        match apply_configuration() {
            Ok(()) => {
                println!("{action_msg} successfully");
                // Confirm states after successful rebuild
                self.registry
                    .confirm_states(&self.module_file.active_modules);
                self.registry
                    .save(MODULES_JSON)
                    .context("failed to save registry after successful rebuild")?;
                Ok(())
            }
            Err(e) => {
                println!("warning: modules in uncertain state due to rebuild failure");
                // Mark relevant modules as uncertain
                self.registry
                    .mark_uncertain(&self.module_file.active_modules);
                self.registry
                    .save(MODULES_JSON)
                    .context("failed to save registry after rebuild failure")?;
                Err(e)
            }
        }
    }

    // Enable modules with state tracking
    pub fn enable_modules(&mut self, modules: &[String], force: bool) -> Result<bool> {
        let mut changes = false;

        // Display status and mark modules for change
        for module in modules {
            let current_state = self.get_effective_state(module);

            match current_state {
                ModuleState::Enabled => {
                    println!("module {module} is already enabled");
                }
                ModuleState::Uncertain => {
                    println!("warning: module {module} is in an uncertain state");
                    changes = true;
                }
                ModuleState::Disabled => {
                    self.registry.set_state(module, ModuleState::Uncertain);
                    changes = true;
                }
            }
        }

        // Update the module file
        let file_changes = self.module_file.enable_modules(modules);
        changes = changes || file_changes;

        // If changes were made or force is set, apply them
        if changes || force {
            self.apply_changes(force, "modules enabled")?;
        } else {
            println!("no changes needed, skipping rebuild");
        }

        Ok(changes)
    }

    // Disable modules with state tracking
    pub fn disable_modules(&mut self, modules: &[String], force: bool) -> Result<bool> {
        let mut changes = false;

        // Display status and mark modules for change
        for module in modules {
            let current_state = self.get_effective_state(module);

            match current_state {
                ModuleState::Enabled => {
                    println!("disabling module {module}...");
                    self.registry.set_state(module, ModuleState::Uncertain);
                    changes = true;
                }
                ModuleState::Uncertain => {
                    println!("warning: module {module} is in an uncertain state");
                    changes = true;
                }
                ModuleState::Disabled => {
                    println!("module {module} is already disabled");
                }
            }
        }

        // Update the module file
        let file_changes = self.module_file.disable_modules(modules);
        changes = changes || file_changes;

        // If changes were made or force is set, apply them
        if changes || force {
            self.apply_changes(force, "modules disabled")?;
        } else {
            println!("no changes needed, skipping rebuild");
        }

        Ok(changes)
    }

    // Reset to base system with state tracking
    pub fn reset(&mut self, force: bool) -> Result<()> {
        println!("resetting to base system...");

        // If we already have an empty state and force is false, skip
        if self.module_file.active_modules.is_empty() && !force {
            println!("system already at base state, skipping rebuild");
            return Ok(());
        }

        // Mark all active modules as uncertain
        self.registry
            .mark_uncertain(&self.module_file.active_modules);

        // Create empty module file
        self.module_file = ModuleFile::empty();

        // Apply changes - use the force parameter passed to the method
        self.apply_changes(force, "system reset")
    }

    // Verify that modules exist in the registry
    pub fn verify_modules_exist(&self, modules: &[String]) -> bool {
        self.registry.verify_modules_exist(modules)
    }

    // Rebuild the system with currently enabled modules
    pub fn rebuild(&mut self, force: bool) -> Result<()> {
        if self.module_file.active_modules.is_empty() && !force {
            println!("no active modules to rebuild");
            return Ok(());
        }

        println!("rebuilding system with current modules:");

        // Display currently enabled modules
        if self.module_file.active_modules.is_empty() {
            println!("  (base system only)");
        } else {
            for module in &self.module_file.active_modules {
                println!("  - {module}");
            }
        }

        // Apply changes
        self.apply_changes(force, "system rebuilt")
    }
}
