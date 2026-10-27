#[cfg(test)]
mod tests {
    use anyhow::Result;
    use runtime_modules::{
        Module, ModuleFile, ModuleRegistry, ModuleState, OperationLock, in_sync, read_applied,
        write_atomic,
    };
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Helper to create a test registry
    fn create_test_registry() -> ModuleRegistry {
        let modules = vec![
            Module {
                name: "test1".to_string(),
                path: "/path/to/test1".to_string(),
                desc: String::new(),
                post_enable: None,
                post_disable: None,
            },
            Module {
                name: "test2".to_string(),
                path: "/path/to/test2".to_string(),
                desc: String::new(),
                post_enable: None,
                post_disable: None,
            },
            Module {
                name: "test3".to_string(),
                path: "/path/to/test3".to_string(),
                desc: String::new(),
                post_enable: None,
                post_disable: None,
            },
        ];

        let mut registry = ModuleRegistry::new(modules);
        registry.init_lookup();
        registry
    }

    // Test registry initialization
    #[test]
    fn test_registry_init_lookup() {
        let modules = vec![
            Module {
                name: "test1".to_string(),
                path: "/path/to/test1".to_string(),
                desc: String::new(),
                post_enable: None,
                post_disable: None,
            },
            Module {
                name: "test2".to_string(),
                path: "/path/to/test2".to_string(),
                desc: String::new(),
                post_enable: None,
                post_disable: None,
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

        write!(temp_file, "{json_content}")?;

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

        write!(temp_file, "{json_content}")?;

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

    // The state of `module` given a state file and an applied marker, as the
    // manager derives it; None stands for a missing file
    fn state_of(
        state_file: Option<&str>,
        marker: Option<&str>,
        module: &str,
    ) -> Result<ModuleState> {
        let dir = tempfile::tempdir()?;
        let state_path = dir.path().join("state.json");
        let marker_path = dir.path().join("runtime-modules-enabled");
        if let Some(content) = state_file {
            std::fs::write(&state_path, content)?;
        }
        if let Some(content) = marker {
            std::fs::write(&marker_path, content)?;
        }

        let requested = ModuleFile::from_file(&state_path)?;
        let applied = read_applied(&marker_path)?;
        Ok(ModuleState::of(
            requested.is_module_enabled(module),
            applied.map(|applied| applied.iter().any(|name| name == module)),
        ))
    }

    #[test]
    fn test_state_enabled_when_requested_and_applied() -> Result<()> {
        let state = state_of(
            Some(r#"{"enabled":["steam"]}"#),
            Some(r#"{"enabled":["steam"]}"#),
            "steam",
        )?;
        assert_eq!(state, ModuleState::Enabled);
        Ok(())
    }

    #[test]
    fn test_state_disabled_on_base_system() -> Result<()> {
        // After a reboot or a base rebuild: no state file, no marker
        assert_eq!(state_of(None, None, "steam")?, ModuleState::Disabled);
        Ok(())
    }

    #[test]
    fn test_state_disabled_when_neither_requested_nor_applied() -> Result<()> {
        let state = state_of(
            Some(r#"{"enabled":["docker"]}"#),
            Some(r#"{"enabled":["docker"]}"#),
            "steam",
        )?;
        assert_eq!(state, ModuleState::Disabled);
        Ok(())
    }

    #[test]
    fn test_state_uncertain_when_enable_never_activated() -> Result<()> {
        // Rebuild interrupted, failed before activation, or still running
        let state = state_of(Some(r#"{"enabled":["steam"]}"#), None, "steam")?;
        assert_eq!(state, ModuleState::Uncertain);
        let state = state_of(
            Some(r#"{"enabled":["steam"]}"#),
            Some(r#"{"enabled":[]}"#),
            "steam",
        )?;
        assert_eq!(state, ModuleState::Uncertain);
        Ok(())
    }

    #[test]
    fn test_state_enabled_when_rebuild_failed_after_activation() -> Result<()> {
        // A unit failing to restart makes nixos-rebuild fail after /etc was switched
        let state = state_of(
            Some(r#"{"enabled":["steam"]}"#),
            Some(r#"{"enabled":["steam"]}"#),
            "steam",
        )?;
        assert_eq!(state, ModuleState::Enabled);
        Ok(())
    }

    #[test]
    fn test_state_uncertain_when_disable_never_activated() -> Result<()> {
        let state = state_of(
            Some(r#"{"enabled":[]}"#),
            Some(r#"{"enabled":["steam"]}"#),
            "steam",
        )?;
        assert_eq!(state, ModuleState::Uncertain);
        Ok(())
    }

    #[test]
    fn test_state_uncertain_with_marker_without_module_list() -> Result<()> {
        // Systems built before the marker recorded its modules
        assert_eq!(
            state_of(Some(r#"{"enabled":["steam"]}"#), Some("true"), "steam")?,
            ModuleState::Uncertain
        );
        assert_eq!(
            state_of(Some(r#"{"enabled":[]}"#), Some("true"), "steam")?,
            ModuleState::Uncertain
        );
        Ok(())
    }

    #[test]
    fn test_retain_known_drops_removed_modules() {
        let registry = create_test_registry();
        let mut module_file = ModuleFile {
            active_modules: vec!["test1".to_string(), "gone".to_string(), "test3".to_string()],
        };

        assert_eq!(
            module_file.retain_known(&registry),
            vec!["gone".to_string()]
        );
        assert_eq!(
            module_file.active_modules,
            vec!["test1".to_string(), "test3".to_string()]
        );
        assert!(module_file.retain_known(&registry).is_empty());
    }

    #[test]
    fn test_in_sync() {
        let a = vec!["steam".to_string(), "docker".to_string()];
        let b = vec!["docker".to_string(), "steam".to_string()];
        assert!(in_sync(&a, Some(&b)));
        assert!(in_sync(&[], Some(&[])));
        assert!(!in_sync(&a, Some(&b[..1])));
        assert!(!in_sync(&[], None));
    }

    #[test]
    fn test_write_atomic_replaces_whole_file() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir()?;
        let path = dir.path().join("state.json");
        std::fs::write(&path, "a much longer previous content")?;

        write_atomic(&path, "{}")?;

        assert_eq!(std::fs::read_to_string(&path)?, "{}");
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o644
        );
        assert_eq!(std::fs::read_dir(dir.path())?.count(), 1);
        Ok(())
    }

    #[test]
    fn test_operation_lock_is_exclusive() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("lock");

        let held = OperationLock::try_acquire(&path)?;
        assert!(held.is_some());
        assert!(OperationLock::try_acquire(&path)?.is_none());

        drop(held);
        assert!(OperationLock::try_acquire(&path)?.is_some());
        Ok(())
    }
}
