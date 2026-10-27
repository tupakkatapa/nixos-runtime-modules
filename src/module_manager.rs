use crate::system::apply_configuration;
use anyhow::{Context, Result};
use runtime_modules::{
    ModuleFile, ModuleRegistry, ModuleState, ModuleStatus, in_sync, read_applied,
};

// Constants
const MODULES_JSON: &str = "/run/runtime-modules/modules.json";
const STATE_FILE: &str = "/run/runtime-modules/state.json";
const APPLIED_MARKER: &str = "/etc/runtime-modules-enabled";

// ModuleManager handles the business logic
pub struct ModuleManager {
    registry: ModuleRegistry,
    module_file: ModuleFile,
    applied: Option<Vec<String>>,
}

impl ModuleManager {
    // Initialize the manager by loading the registry, the requested modules and
    // the modules the running system was built with
    pub fn new() -> Result<Self> {
        Ok(Self {
            registry: ModuleRegistry::from_file(MODULES_JSON)
                .context("failed to load module registry")?,
            module_file: ModuleFile::from_file(STATE_FILE).context("failed to load module file")?,
            applied: read_applied(APPLIED_MARKER).context("failed to read applied modules")?,
        })
    }

    fn state(&self, module: &str) -> ModuleState {
        ModuleState::of(
            self.module_file.is_module_enabled(module),
            self.applied
                .as_ref()
                .map(|applied| applied.iter().any(|name| name == module)),
        )
    }

    // Get status for specific modules
    pub fn get_status(&self, modules: &[String]) -> Vec<ModuleStatus> {
        modules
            .iter()
            .map(|module| {
                let state = self.state(module);

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
                        post_enable: registry_module.post_enable.clone(),
                        post_disable: registry_module.post_disable.clone(),
                    }
                } else {
                    // Fallback if module not found
                    ModuleStatus {
                        name: module.clone(),
                        path: String::new(),
                        state,
                        desc: String::new(),
                        post_enable: None,
                        post_disable: None,
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
            .map(|module| ModuleStatus {
                name: module.name.clone(),
                path: module.path.clone(),
                state: self.state(&module.name),
                desc: module.desc.clone(),
                post_enable: module.post_enable.clone(),
                post_disable: module.post_disable.clone(),
            })
            .collect()
    }

    // Rebuild with the requested modules, unless the running system already has
    // exactly them. The state file is the input of the rebuild, so it is saved
    // first; the outcome is read back from the running system, not recorded.
    fn apply_changes(&self, force: bool, action_msg: &str) -> Result<()> {
        if !force && in_sync(&self.module_file.active_modules, self.applied.as_deref()) {
            println!("no changes needed, skipping rebuild");
            return Ok(());
        }

        self.module_file
            .save(STATE_FILE)
            .with_context(|| format!("failed to save state file before {action_msg}"))?;
        println!("saved state to '{STATE_FILE}'");

        apply_configuration()?;
        println!("{action_msg} successfully");
        Ok(())
    }

    // Enable modules, rebuilding unless the running system already has them
    pub fn enable_modules(&mut self, modules: &[String], force: bool) -> Result<()> {
        for module in modules {
            match self.state(module) {
                ModuleState::Enabled => println!("'{module}' module is already enabled"),
                ModuleState::Uncertain => {
                    println!("warning: '{module}' module is in an uncertain state");
                }
                ModuleState::Disabled => {}
            }
        }

        self.module_file.enable_modules(modules);
        self.apply_changes(force, "modules enabled")
    }

    // Disable modules, rebuilding unless the running system already lacks them
    pub fn disable_modules(&mut self, modules: &[String], force: bool) -> Result<()> {
        for module in modules {
            match self.state(module) {
                ModuleState::Enabled => println!("disabling '{module}' module..."),
                ModuleState::Uncertain => {
                    println!("warning: '{module}' module is in an uncertain state");
                }
                ModuleState::Disabled => println!("'{module}' module is already disabled"),
            }
        }

        self.module_file.disable_modules(modules);
        self.apply_changes(force, "modules disabled")
    }

    // Reset to the base system
    pub fn reset(&mut self, force: bool) -> Result<()> {
        println!("resetting to base system...");
        self.module_file = ModuleFile::empty();
        self.apply_changes(force, "system reset")
    }

    // Verify that modules exist in the registry
    pub fn verify_modules_exist(&self, modules: &[String]) -> bool {
        self.registry.verify_modules_exist(modules)
    }

    // Rebuild the system with currently enabled modules
    pub fn rebuild(&self, force: bool) -> Result<()> {
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

        self.apply_changes(true, "system rebuilt")
    }
}
