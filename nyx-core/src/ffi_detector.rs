/// Utilities to help detect whether we're running under a foreign function interface
/// (FFI) boundary such as mobile embedding. This is best-effort and purely heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiEnvironment {
    Native,
    Mobile,
    Unknown,
}

/// Heuristic based on common environment variables set by mobile or embedded hosts.
pub fn detect() -> FfiEnvironment {
    let vars = [
        "ANDROID_ARGUMENT",    // python-for-android
        "JNI_WRAPPER",         // custom JNI bridges
        "IOS_BUNDLE",          // hypothetical markers
        "NYX_MOBILE_EMBEDDED", // project-specific
    ];
    for k in vars {
        if std::env::var_os(k).is_some() {
            return FfiEnvironment::Mobile;
        }
    }
    FfiEnvironment::Native
}

#[cfg(test)]
mod test_s {
    use super::*;
    #[test]
    fn detect_defaultsnative() {
        // In CI/desktop usually no mobile vars present
        let x = detect();
        assert!(matches!(
            x,
            FfiEnvironment::Native | FfiEnvironment::Unknown
        ));
    }
}
