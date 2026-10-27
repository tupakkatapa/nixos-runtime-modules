use crate::system::apply_configuration;
use anyhow::{Context, Result};
use runtime_modules::{
    ModuleFile, ModuleRegistry, ModuleState, ModuleStatus, OperationLock, in_sync, read_applied,
};

// Constants
const MODULES_JSON: &str = "/run/runtime-modules/modules.json";
const STATE_FILE: &str = "/run/runtime-modules/state.json";
const LOCK_FILE: &str = "/run/runtime-modules/lock";
const APPLIED_MARKER: &str = "/etc/runtime-modules-enabled";

// ModuleManager handles the business logic
pub struct ModuleManager {
    registry: ModuleRegistry,
    module_file: ModuleFile,
    applied: Option<Vec<String>>,
    _lock: Option<OperationLock>,
}

impl ModuleManager {
    // Initialize the manager by loading the registry, the requested modules and
    // the modules the running system was built with
    pub fn new() -> Result<Self> {
        Self::load(None)
    }

    // Like new, but holds the operation lock until dropped, so that changes
    // to the system happen one at a time
    pub fn new_locked() -> Result<Self> {
        let lock = OperationLock::acquire(LOCK_FILE, || {
            println!("waiting for another runtime-modules operation to finish...");
        })?;
        Self::load(Some(lock))
    }

    fn load(lock: Option<OperationLock>) -> Result<Self> {
        let registry =
            ModuleRegistry::from_file(MODULES_JSON).context("failed to load module registry")?;
        let mut module_file =
            ModuleFile::from_file(STATE_FILE).context("failed to load module file")?;

        // The base configuration no longer defines these, so no rebuild can apply them
        for module in module_file.retain_known(&registry) {
            println!("warning: '{module}' module no longer exists, dropping it");
        }

        Ok(Self {
            registry,
            module_file,
            applied: read_applied(APPLIED_MARKER).context("failed to read applied modules")?,
            _lock: lock,
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

    fn status(&self, name: &str) -> ModuleStatus {
        let module = self
            .registry
            .get_lookup_map()
            .and_then(|map| map.get(name))
            .map(|&index| &self.registry.modules[index]);

        ModuleStatus {
            name: name.to_string(),
            path: module.map(|m| m.path.clone()).unwrap_or_default(),
            state: self.state(name),
            desc: module.map(|m| m.desc.clone()).unwrap_or_default(),
            post_enable: module.and_then(|m| m.post_enable.clone()),
            post_disable: module.and_then(|m| m.post_disable.clone()),
        }
    }

    // Get status for specific modules
    pub fn get_status(&self, modules: &[String]) -> Vec<ModuleStatus> {
        modules.iter().map(|module| self.status(module)).collect()
    }

    // Get status for all modules
    pub fn get_all_status(&self) -> Vec<ModuleStatus> {
        self.registry
            .modules
            .iter()
            .map(|module| self.status(&module.name))
            .collect()
    }

    fn warn_uncertain(&self, module: &str) {
        match &self.applied {
            None => println!(
                "warning: '{module}' module state is unknown, the running system does not record its modules"
            ),
            Some(applied) if applied.iter().any(|name| name == module) => {
                println!("warning: '{module}' module is active but not requested");
            }
            Some(_) => println!("warning: '{module}' module is requested but not active"),
        }
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
                ModuleState::Uncertain => self.warn_uncertain(module),
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
                ModuleState::Uncertain => self.warn_uncertain(module),
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
