/// Local-only Desktop flavor: no Remote, kanban, or relay.
///
/// Enabled by compile-time `lite` feature and/or runtime `VK_LITE=1`.
pub fn is_lite_mode() -> bool {
    cfg!(feature = "lite") || env_flag_enabled("VK_LITE")
}

fn env_flag_enabled(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::env_flag_enabled;

    #[test]
    fn env_flag_accepts_common_truthy_values() {
        const KEY: &str = "VK_LITE_TEST_FLAG";
        unsafe {
            std::env::set_var(KEY, "1");
        }
        assert!(env_flag_enabled(KEY));
        unsafe {
            std::env::set_var(KEY, "true");
        }
        assert!(env_flag_enabled(KEY));
        unsafe {
            std::env::set_var(KEY, "0");
        }
        assert!(!env_flag_enabled(KEY));
        unsafe {
            std::env::remove_var(KEY);
        }
        assert!(!env_flag_enabled(KEY));
    }
}
