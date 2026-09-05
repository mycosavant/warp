use std::path::PathBuf;

use lsp_types::Uri;

use crate::config::{lsp_uri_to_path, path_to_lsp_uri};

// Unix-specific tests use Unix paths
#[cfg(not(windows))]
mod unix_tests {
    use super::*;

    #[test]
    fn test_lsp_uri_to_path_basic() {
        let uri: Uri = "file:///Users/test/project/src/main.rs".parse().unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(path, PathBuf::from("/Users/test/project/src/main.rs"));
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_at_symbol() {
        // %40 is the URL encoding for @
        let uri: Uri = "file:///Users/test/node_modules/%40firebase/auth/dist/index.d.ts"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/Users/test/node_modules/@firebase/auth/dist/index.d.ts")
        );
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_spaces() {
        // %20 is the URL encoding for space
        let uri: Uri = "file:///Users/test/My%20Project/src/main.rs"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(path, PathBuf::from("/Users/test/My Project/src/main.rs"));
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_multiple_special_chars() {
        // Test multiple encoded characters: @ (%40), space (%20), # (%23)
        let uri: Uri = "file:///Users/test/%40scope/my%20package%23v1/index.ts"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/Users/test/@scope/my package#v1/index.ts")
        );
    }

    #[test]
    fn test_path_to_lsp_uri_basic() {
        let path = PathBuf::from("/Users/test/project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(uri.as_str(), "file:///Users/test/project/src/main.rs");
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_spaces() {
        let path = PathBuf::from("/Users/test/My Project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(uri.as_str(), "file:///Users/test/My%20Project/src/main.rs");
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_non_ascii() {
        let path = PathBuf::from("/Users/관리자/project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert!(uri.as_str().starts_with("file:///Users/%"));
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_accented_chars() {
        let path = PathBuf::from("/Users/José/project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert!(uri.as_str().starts_with("file:///Users/Jos%"));
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_hash() {
        let path = PathBuf::from("/Users/test/my#project/src/main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(uri.as_str(), "file:///Users/test/my%23project/src/main.rs");
    }

    #[test]
    fn test_roundtrip_path_to_uri_to_path() {
        let original_path = PathBuf::from("/Users/test/project/src/main.rs");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }

    #[test]
    fn test_roundtrip_non_ascii_path() {
        let original_path = PathBuf::from("/Users/관리자/project/src/main.rs");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }

    #[test]
    fn test_roundtrip_path_with_spaces() {
        let original_path = PathBuf::from("/Users/test/My Project/src/main.rs");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }

    #[test]
    fn test_path_to_lsp_uri_encodes_brackets() {
        let path = PathBuf::from("/Users/test/routes/blog/[slug].tsx");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(
            uri.as_str(),
            "file:///Users/test/routes/blog/%5Bslug%5D.tsx"
        );
    }

    #[test]
    fn test_roundtrip_path_with_brackets() {
        let original_path = PathBuf::from("/Users/test/routes/[id]/[slug].tsx");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }
}

// Windows-specific tests use Windows paths
#[cfg(windows)]
mod windows_tests {
    use super::*;

    #[test]
    fn test_lsp_uri_to_path_basic() {
        let uri: Uri = "file:///C:/Users/test/project/src/main.rs".parse().unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("C:\\Users\\test\\project\\src\\main.rs")
        );
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_at_symbol() {
        // %40 is the URL encoding for @
        let uri: Uri = "file:///C:/Users/test/node_modules/%40firebase/auth/dist/index.d.ts"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("C:\\Users\\test\\node_modules\\@firebase\\auth\\dist\\index.d.ts")
        );
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_spaces() {
        // %20 is the URL encoding for space
        let uri: Uri = "file:///C:/Users/test/My%20Project/src/main.rs"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("C:\\Users\\test\\My Project\\src\\main.rs")
        );
    }

    #[test]
    fn test_lsp_uri_to_path_decodes_multiple_special_chars() {
        // Test multiple encoded characters: @ (%40), space (%20), # (%23)
        let uri: Uri = "file:///C:/Users/test/%40scope/my%20package%23v1/index.ts"
            .parse()
            .unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from("C:\\Users\\test\\@scope\\my package#v1\\index.ts")
        );
    }

    #[test]
    fn test_path_to_lsp_uri_basic() {
        let path = PathBuf::from("C:\\Users\\test\\project\\src\\main.rs");
        let uri = path_to_lsp_uri(&path).unwrap();
        assert_eq!(uri.as_str(), "file:///C:/Users/test/project/src/main.rs");
    }

    #[test]
    fn test_roundtrip_path_to_uri_to_path() {
        let original_path = PathBuf::from("C:\\Users\\test\\project\\src\\main.rs");
        let uri = path_to_lsp_uri(&original_path).unwrap();
        let roundtrip_path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(original_path, roundtrip_path);
    }
}

// Platform-independent tests
#[test]
fn test_lsp_uri_to_path_rejects_non_file_uri() {
    let uri: Uri = "https://example.com/path".parse().unwrap();
    let result = lsp_uri_to_path(&uri);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid file URI"));
}

#[test]
fn test_path_to_lsp_uri_rejects_relative_path() {
    let path = PathBuf::from("relative/path/file.rs");
    let result = path_to_lsp_uri(&path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be absolute"));
}

// The WSL mapping is string-to-string and depends on no filesystem, so it is
// asserted on every platform: the Windows build is the one that runs it and
// the Linux build is the one that runs the tests.
mod wsl_tests {
    use std::path::{Path, PathBuf};

    use lsp_types::Uri;

    use crate::config::UriMapper;

    fn mapper() -> UriMapper {
        UriMapper::WslDistro("Ubuntu".to_string())
    }

    #[test]
    fn a_wsl_unc_path_leaves_as_a_linux_file_uri() {
        let uri = mapper()
            .to_uri(Path::new(
                r"\\wsl$\Ubuntu\home\effatha\git\warp\src\main.rs",
            ))
            .unwrap();
        assert_eq!(uri.as_str(), "file:///home/effatha/git/warp/src/main.rs");
    }

    #[test]
    fn every_spelling_of_the_prefix_maps_to_the_same_uri() {
        for spelled in [
            r"\\wsl$\Ubuntu\home\e\a.rs",
            r"\\wsl.localhost\Ubuntu\home\e\a.rs",
            r"\\WSL$\UBUNTU\home\e\a.rs",
            r"\\?\UNC\wsl$\Ubuntu\home\e\a.rs",
        ] {
            let uri = mapper().to_uri(Path::new(spelled)).unwrap();
            assert_eq!(uri.as_str(), "file:///home/e/a.rs", "{spelled}");
        }
    }

    #[test]
    fn spaces_and_brackets_are_percent_encoded_on_the_way_out() {
        let uri = mapper()
            .to_uri(Path::new(r"\\wsl$\ubuntu\home\e\my dir\[slug].tsx"))
            .unwrap();
        assert_eq!(uri.as_str(), "file:///home/e/my%20dir/%5Bslug%5D.tsx");
    }

    #[test]
    fn a_linux_file_uri_comes_back_in_the_canonical_wsl_spelling() {
        let uri: Uri = "file:///home/effatha/git/warp/src/main.rs".parse().unwrap();
        let path = mapper().to_path(&uri).unwrap();
        assert_eq!(
            path,
            PathBuf::from(r"\\wsl$\ubuntu\home\effatha\git\warp\src\main.rs")
        );
        // Pinned to the same normal form the rest of the app keys on, so a
        // definition the server answers with lands on a buffer Warp already
        // holds rather than on a second spelling of it.
        assert_eq!(
            Some(path),
            warp_util::path::canonicalize_wsl_unc_path(Path::new(
                r"\\wsl.localhost\Ubuntu\home\effatha\git\warp\src\main.rs"
            ))
        );
    }

    #[test]
    fn percent_encoding_is_decoded_on_the_way_back() {
        let uri: Uri = "file:///home/e/my%20dir/%5Bslug%5D.tsx".parse().unwrap();
        assert_eq!(
            mapper().to_path(&uri).unwrap(),
            PathBuf::from(r"\\wsl$\ubuntu\home\e\my dir\[slug].tsx")
        );
    }

    #[test]
    fn the_distribution_root_has_no_trailing_separator() {
        let uri: Uri = "file:///".parse().unwrap();
        assert_eq!(
            mapper().to_path(&uri).unwrap(),
            PathBuf::from(r"\\wsl$\ubuntu")
        );
    }

    #[test]
    fn a_path_outside_the_distribution_is_refused_not_guessed() {
        assert!(mapper().to_uri(Path::new(r"C:\Users\e\a.rs")).is_err());
        assert!(
            mapper()
                .to_uri(Path::new(r"\\wsl$\Debian\home\e\a.rs"))
                .is_err(),
            "another distribution's file is not this server's"
        );
    }

    #[test]
    fn a_uri_naming_a_host_is_refused() {
        let uri: Uri = "file://wsl.localhost/Ubuntu/home/e/a.rs".parse().unwrap();
        assert!(mapper().to_path(&uri).is_err());
        let uri: Uri = "https://example.com/a.rs".parse().unwrap();
        assert!(mapper().to_path(&uri).is_err());
    }

    #[test]
    fn round_trip_is_the_identity_on_the_canonical_spelling() {
        let canonical = PathBuf::from(r"\\wsl$\ubuntu\home\e\src\lib.rs");
        let uri = mapper().to_uri(&canonical).unwrap();
        assert_eq!(mapper().to_path(&uri).unwrap(), canonical);
    }
}
