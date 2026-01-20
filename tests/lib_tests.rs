#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use runtime_modules::{Module, ModuleFile, ModuleRegistry, ModuleState};
    use std::io::{self, Write};
    use tempfile::NamedTempFile;

    // Helper to create a test registry
    fn create_test_registry() -> ModuleRegistry {
        let modules = vec![
            Module {
                name: "test1".to_string(),
                path: "/path/to/test1".to_string(),
                desc: String::new(),
                state: ModuleState::Disabled,
            },
            Module {
                name: "test2".to_string(),
                path: "/path/to/test2".to_string(),
                desc: String::new(),
                state: ModuleState::Disabled,
            },
            Module {
                name: "test3".to_string(),
                path: "/path/to/test3".to_string(),
                desc: String::new(),
                state: ModuleState::Disabled,
            },
        ];

        let mut registry = ModuleRegistry::new(modules);
        registry.init_lookup();
        registry
    }

    // Test error context and formatting
    #[test]
    fn test_error_handling() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let wrapped_err = anyhow::Error::new(io_err).context("failed to access file");

        assert!(wrapped_err.to_string().contains("failed to access file"));

        let missing_module_err = anyhow!("module not found: {}", "missing");
        assert!(missing_module_err.to_string().contains("module not found"));
    }

    // Test registry initialization
    #[test]
    fn test_registry_init_lookup() {
        let modules = vec![
            Module {
                name: "test1".to_string(),
                path: "/path/to/test1".to_string(),
                desc: String::new(),
                state: ModuleState::Disabled,
            },
            Module {
                name: "test2".to_string(),
                path: "/path/to/test2".to_string(),
                desc: String::new(),
                state: ModuleState::Disabled,
            },
        ];

        let mut registry = ModuleRegistry::new(modules);

        // Before init, module_map should be None
        assert!(!registry.has_lookup_map());

        registry.init_lookup();

        // After init, module_map should contain both modules
        assert!(registry.has_lookup_map());
        if let Some(map) = registry.get_lookup_map() {
            assert_eq!(map.len(), 2);
            assert_eq!(map.get("test1"), Some(&0));
            assert_eq!(map.get("test2"), Some(&1));
        }
    }

    // Test loading registry from a file
    #[test]
    fn test_registry_from_file() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        let json_content = r#"
        {
            "modules": [
                {"name": "test1", "path": "/path/to/test1"},
                {"name": "test2", "path": "/path/to/test2"}
            ]
        }
        "#;

        write!(temp_file, "{}", json_content)?;

        let registry = ModuleRegistry::from_file(temp_file.path())?;

        assert_eq!(registry.modules.len(), 2);
        assert_eq!(registry.modules[0].name, "test1");
        assert_eq!(registry.modules[1].path, "/path/to/test2");

        // Verify lookup was initialized
        assert!(registry.has_lookup_map());

        Ok(())
    }

    // Test loading ModuleFile from file
    #[test]
    fn test_module_file_from_file() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        let json_content = r#"{"enabled":["test1","test2"]}"#;

        write!(temp_file, "{}", json_content)?;

        let module_file = ModuleFile::from_file(temp_file.path())?;

        assert_eq!(module_file.active_modules.len(), 2);
        assert!(module_file.active_modules.contains(&"test1".to_string()));
        assert!(module_file.active_modules.contains(&"test2".to_string()));

        Ok(())
    }

    // Test parsing edge cases
    #[test]
    fn test_parsing_edge_cases() {
        // Test with empty content (invalid JSON returns empty)
        let empty_modules = ModuleFile::parse_active_modules("");
        assert!(empty_modules.is_empty());

        // Test with invalid JSON
        let invalid_json = ModuleFile::parse_active_modules("not valid json");
        assert!(invalid_json.is_empty());

        // Test with empty enabled array
        let empty_enabled = ModuleFile::parse_active_modules(r#"{"enabled":[]}"#);
        assert!(empty_enabled.is_empty());

        // Test with valid JSON
        let valid_json = ModuleFile::parse_active_modules(r#"{"enabled":["test1"]}"#);
        assert_eq!(valid_json.len(), 1);
        assert_eq!(valid_json[0], "test1");

        // Test with multiple modules
        let multiple = ModuleFile::parse_active_modules(r#"{"enabled":["test1","test2","test3"]}"#);
        assert_eq!(multiple.len(), 3);
    }

    // Property-based test: enabling then disabling should restore original state
    #[test]
    fn test_enable_disable_property() {
        let mut module_file = ModuleFile::empty();

        // Enable a module
        module_file.enable_modules(&["test1".to_string()]);
        assert!(module_file.is_module_enabled("test1"));

        // Disable the same module
        module_file.disable_modules(&["test1".to_string()]);
        assert!(!module_file.is_module_enabled("test1"));
        assert_eq!(module_file.active_modules.len(), 0);

        // Enable multiple modules
        module_file.enable_modules(&["test1".to_string(), "test2".to_string()]);
        assert!(module_file.is_module_enabled("test1"));
        assert!(module_file.is_module_enabled("test2"));

        // Disable in reverse order
        module_file.disable_modules(&["test2".to_string()]);
        assert!(module_file.is_module_enabled("test1"));
        assert!(!module_file.is_module_enabled("test2"));

        module_file.disable_modules(&["test1".to_string()]);
        assert!(!module_file.is_module_enabled("test1"));
        assert_eq!(module_file.active_modules.len(), 0);
    }

    // Test multiple operations sequence
    #[test]
    fn test_multiple_operations() {
        let mut module_file = ModuleFile::empty();
        let _registry = create_test_registry();

        // Enable some modules
        module_file.enable_modules(&["test1".to_string(), "test2".to_string()]);
        assert_eq!(module_file.active_modules.len(), 2);

        // Generate content
        let content = module_file.generate_content();
        assert!(content.contains("test1"));
        assert!(content.contains("test2"));

        // Disable one module
        module_file.disable_modules(&["test1".to_string()]);
        assert_eq!(module_file.active_modules.len(), 1);

        // Generate updated content
        let updated_content = module_file.generate_content();
        assert!(!updated_content.contains("test1"));
        assert!(updated_content.contains("test2"));

        // Enable a different module
        module_file.enable_modules(&["test3".to_string()]);
        assert_eq!(module_file.active_modules.len(), 2);

        // Final content should have test2 and test3
        let final_content = module_file.generate_content();
        assert!(final_content.contains("test2"));
        assert!(final_content.contains("test3"));
    }

    // Test behavior with duplicate module names
    #[test]
    fn test_duplicate_modules() {
        let mut module_file = ModuleFile::empty();

        // Enable a module multiple times
        module_file.enable_modules(&["test1".to_string()]);
        let first_result = module_file.enable_modules(&["test1".to_string()]);

        // Should return false (no changes) on second attempt
        assert!(!first_result);
        assert_eq!(module_file.active_modules.len(), 1);

        // Enable multiple with duplicates
        let multi_result = module_file.enable_modules(&["test1".to_string(), "test2".to_string()]);

        // Should return true (changes made) because test2 was added
        assert!(multi_result);
        assert_eq!(module_file.active_modules.len(), 2);

        // Try duplicate in the same list
        let dup_list_result =
            module_file.enable_modules(&["test3".to_string(), "test3".to_string()]);

        // Should only add test3 once
        assert!(dup_list_result);
        assert_eq!(module_file.active_modules.len(), 3);

        // Count occurrences of test3
        let test3_count = module_file
            .active_modules
            .iter()
            .filter(|&name| name == "test3")
            .count();
        assert_eq!(test3_count, 1);
    }

    // Test module state management
    #[test]
    fn test_state_management() {
        let mut registry = create_test_registry();

        // Test initial state
        assert_eq!(registry.get_state("test1"), ModuleState::Disabled);

        // Test setting state
        registry.set_state("test1", ModuleState::Enabled);
        assert_eq!(registry.get_state("test1"), ModuleState::Enabled);

        registry.set_state("test2", ModuleState::Uncertain);
        assert_eq!(registry.get_state("test2"), ModuleState::Uncertain);

        // Test confirm states
        let active_modules = vec!["test1".to_string(), "test3".to_string()];
        registry.confirm_states(&active_modules);

        // Should set test1 and test3 to Enabled, test2 to Disabled
        assert_eq!(registry.get_state("test1"), ModuleState::Enabled);
        assert_eq!(registry.get_state("test2"), ModuleState::Disabled);
        assert_eq!(registry.get_state("test3"), ModuleState::Enabled);

        // Test mark uncertain
        registry.mark_uncertain(&["test1".to_string()]);
        assert_eq!(registry.get_state("test1"), ModuleState::Uncertain);
    }
}
