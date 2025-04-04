use crate::system::apply_configuration;
use runtime_module::{ModuleError, ModuleFile, ModuleRegistry, ModuleStatus};

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
    pub fn new() -> Result<Self, ModuleError> {
        let registry = ModuleRegistry::from_file(MODULES_JSON)?;
        let module_file = ModuleFile::from_file(MODULES_FILE)?;

        Ok(Self {
            registry,
            module_file,
        })
    }

    // Get status for specific modules
    pub fn get_status(&self, modules: &[String]) -> Vec<ModuleStatus> {
        modules
            .iter()
            .map(|module| {
                let enabled = self.module_file.is_module_enabled(module);
                let path = self.registry.get_module_path(module).unwrap_or_default();

                ModuleStatus {
                    name: module.clone(),
                    path,
                    enabled,
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
                let enabled = self.module_file.is_module_enabled(name);
                ModuleStatus {
                    name: name.clone(),
                    path: module.path.clone(),
                    enabled,
                }
            })
            .collect()
    }

    // Enable modules and apply changes
    pub fn enable_modules(&mut self, modules: &[String], force: bool) -> Result<bool, ModuleError> {
        // Display status for modules that are already enabled
        for module in modules {
            if self.module_file.is_module_enabled(module) {
                println!("module {module} is already enabled");
            }
        }

        // Enable the specified modules
        let changes = self.module_file.enable_modules(modules);

        // If changes were made or force flag is set, save and apply
        if changes || force {
            self.module_file.save(MODULES_FILE, &self.registry)?;
            println!("generated modules file at '{MODULES_FILE}'");

            apply_configuration()?;
            println!("modules enabled successfully");
        } else {
            println!("no changes needed, skipping rebuild");
        }

        Ok(changes)
    }

    // Disable modules and apply changes
    pub fn disable_modules(
        &mut self,
        modules: &[String],
        force: bool,
    ) -> Result<bool, ModuleError> {
        // Display status for each module
        for module in modules {
            if self.module_file.is_module_enabled(module) {
                println!("disabling module {module}...");
            } else {
                println!("module {module} is already disabled");
            }
        }

        // Disable the specified modules
        let changes = self.module_file.disable_modules(modules);

        // If changes were made or force flag is set, save and apply
        if changes || force {
            self.module_file.save(MODULES_FILE, &self.registry)?;
            println!("generated modules file at '{MODULES_FILE}'");

            apply_configuration()?;
            println!("modules disabled successfully");
        } else {
            println!("no changes needed, skipping rebuild");
        }

        Ok(changes)
    }

    // Reset to base system (disable all modules)
    pub fn reset(&mut self, force: bool) -> Result<(), ModuleError> {
        println!("resetting to base system...");

        // If we already have an empty state and force is false, skip
        if self.module_file.active_modules.is_empty() && !force {
            println!("system already at base state, skipping rebuild");
            return Ok(());
        }

        self.module_file = ModuleFile::empty();
        self.module_file.save(MODULES_FILE, &self.registry)?;
        println!("generated modules file at '{MODULES_FILE}'");

        apply_configuration()?;
        println!("system reset successfully");
        Ok(())
    }

    // Verify that modules exist in the registry
    pub fn verify_modules_exist(&self, modules: &[String]) -> bool {
        self.registry.verify_modules_exist(modules)
    }

    // Rebuild the system with currently enabled modules
    pub fn rebuild(&self, force: bool) -> Result<(), ModuleError> {
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

        apply_configuration()?;
        println!("system rebuilt successfully");
        Ok(())
    }
}
