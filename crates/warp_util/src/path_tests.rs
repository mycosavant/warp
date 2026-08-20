use super::*;

#[test]
fn test_user_friendly_path_with_home() {
    let home = "/Users/blue";
    assert_eq!(
        user_friendly_path("/Users/blue", Some(home)),
        "~".to_string(),
    );
    assert_eq!(
        user_friendly_path("/Users/blue/warp", Some(home)),
        "~/warp".to_string(),
    );
    assert_eq!(
        user_friendly_path("/Users/admin/warp", Some(home)),
        "/Users/admin/warp".to_string(),
    );
}

#[test]
fn test_to_relative_path() {
    use std::path::Path;

    use super::to_relative_path;

    // Basic relative path conversion
    #[cfg(not(windows))]
    {
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john/projects/app/src/main.rs"),
                Path::new("/Users/john/projects")
            ),
            Some("app/src/main.rs".to_string())
        );

        // Same directory
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john/projects"),
                Path::new("/Users/john/projects")
            ),
            Some(".".to_string())
        );

        // Parent directory
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john"),
                Path::new("/Users/john/projects")
            ),
            Some("..".to_string())
        );

        // Nested parent
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users"),
                Path::new("/Users/john/projects")
            ),
            Some("../..".to_string())
        );

        // Cross-branch paths
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john/documents/file.txt"),
                Path::new("/Users/john/projects")
            ),
            Some("../documents/file.txt".to_string())
        );

        // Root to subdirectory
        assert_eq!(
            to_relative_path(false, Path::new("/var/log/system.log"), Path::new("/")),
            Some("var/log/system.log".to_string())
        );

        // Handles paths that would have leading slashes correctly
        assert_eq!(
            to_relative_path(false, Path::new("/home/user/file.txt"), Path::new("/home")),
            Some("user/file.txt".to_string())
        );

        // Test with current directory references that should be cleaned
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john/projects/./app/src/main.rs"),
                Path::new("/Users/john/projects")
            ),
            Some("app/src/main.rs".to_string()),
        );
    }

    #[cfg(windows)]
    {
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john/projects/app/src/main.rs"),
                Path::new("/Users/john/projects")
            ),
            Some("app\\src\\main.rs".to_string())
        );

        // Same directory
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john/projects"),
                Path::new("/Users/john/projects")
            ),
            Some(".".to_string())
        );

        // Parent directory
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john"),
                Path::new("/Users/john/projects")
            ),
            Some("..".to_string())
        );

        // Nested parent
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users"),
                Path::new("/Users/john/projects")
            ),
            Some("..\\..".to_string())
        );

        // Cross-branch paths
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john/documents/file.txt"),
                Path::new("/Users/john/projects")
            ),
            Some("..\\documents\\file.txt".to_string())
        );

        // Root to subdirectory
        assert_eq!(
            to_relative_path(false, Path::new("/var/log/system.log"), Path::new("/")),
            Some("var\\log\\system.log".to_string())
        );

        // Handles paths that would have leading slashes correctly
        assert_eq!(
            to_relative_path(false, Path::new("/home/user/file.txt"), Path::new("/home")),
            Some("user\\file.txt".to_string())
        );

        // Test with current directory references that should be cleaned
        assert_eq!(
            to_relative_path(
                false,
                Path::new("/Users/john/projects/./app/src/main.rs"),
                Path::new("/Users/john/projects")
            ),
            Some("app\\src\\main.rs".to_string())
        );

        // Windows paths - different drives should return None
        assert_eq!(
            to_relative_path(
                /* is_wsl */ false,
                Path::new("D:\\projects\\app"),
                Path::new("C:\\workspace")
            ),
            None,
        );

        // Windows paths - same drive
        assert_eq!(
            to_relative_path(
                /* is_wsl */ false,
                Path::new("C:\\projects\\app\\src\\main.rs"),
                Path::new("C:\\projects")
            ),
            Some("app\\src\\main.rs".to_string())
        );

        // Windows paths - same drive -- WSL is disabled for now
        assert_eq!(
            to_relative_path(
                /* is_wsl */ true,
                Path::new("C:\\projects\\app\\src\\main.rs"),
                Path::new("C:\\projects")
            ),
            None
        );
    }
}

#[test]
fn test_normalize_relative_path_for_glob() {
    use std::path::Path;

    assert_eq!(
        normalize_relative_path_for_glob(Path::new("app/src/main.rs")),
        "app/src/main.rs"
    );
    assert_eq!(
        normalize_relative_path_for_glob(Path::new("./app/src/main.rs")),
        "app/src/main.rs"
    );
    assert_eq!(
        normalize_relative_path_for_glob(Path::new("../app/src/main.rs")),
        "app/src/main.rs"
    );
    assert_eq!(normalize_relative_path_for_glob(Path::new("..")), "");
    assert_eq!(normalize_relative_path_for_glob(Path::new("")), "");
}

#[test]
fn test_posix_escape() {
    let shell_family = ShellFamily::Posix;
    assert_eq!(
        shell_family.escape("~/test_dir/library% 1$2"),
        "\\~/test_dir/library%\\ 1\\$2"
    );
    assert_eq!(shell_family.escape("あい"), "あい");
    assert_eq!(shell_family.escape("abc \n \t"), "abc\\ \\\n\\ \\\t");
    assert_eq!(shell_family.escape(""), "''");
    assert_eq!(
        shell_family.escape("foo '\"' bar"),
        "foo\\ \\'\\\"\\'\\ bar"
    );
}

#[test]
fn test_powershell_escape() {
    let shell_family = ShellFamily::PowerShell;
    assert_eq!(
        shell_family.escape("~/test_dir/library% 1$2"),
        "~/test_dir/library%` 1`$2"
    );
    assert_eq!(shell_family.escape("あい"), "あい");
    assert_eq!(shell_family.escape("abc \n \t"), "abc` `\n` `\t");
    assert_eq!(shell_family.escape(""), "''");
    assert_eq!(shell_family.escape("foo '\"' bar"), "foo` `'`\"`'` bar");
}

#[test]
fn test_posix_unescape() {
    let shell_family = ShellFamily::Posix;
    // Escaped spaces
    assert_eq!(shell_family.unescape("my\\ file.txt"), "my file.txt");
    // Multiple escaped characters
    assert_eq!(
        shell_family.unescape("path/to/my\\ file\\ \\(1\\).txt"),
        "path/to/my file (1).txt"
    );
    // No escaping needed — returns borrowed
    assert!(matches!(
        shell_family.unescape("simple.txt"),
        std::borrow::Cow::Borrowed(_)
    ));
    // Trailing backslash kept as-is
    assert_eq!(shell_family.unescape("trailing\\"), "trailing\\");
    // Roundtrip: unescape(escape(x)) == x
    let original = "hello world $HOME 'quotes'";
    assert_eq!(
        shell_family.unescape(&shell_family.escape(original)),
        original
    );
}

#[test]
fn test_powershell_unescape() {
    let shell_family = ShellFamily::PowerShell;
    // Escaped spaces
    assert_eq!(shell_family.unescape("my` file.txt"), "my file.txt");
    // Multiple escaped characters
    assert_eq!(shell_family.unescape("path` `$var"), "path $var");
    // No escaping needed — returns borrowed
    assert!(matches!(
        shell_family.unescape("simple.txt"),
        std::borrow::Cow::Borrowed(_)
    ));
    // Roundtrip: unescape(escape(x)) == x
    let original = "hello world $HOME";
    assert_eq!(
        shell_family.unescape(&shell_family.escape(original)),
        original
    );
}

#[test]
fn test_clean_path() {
    assert_eq!(
        CleanPathResult::with_line_and_column_number("Cargo.toml:10:5"),
        CleanPathResult {
            path: "Cargo.toml".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 10,
                column_num: Some(5)
            }),
        }
    );

    assert_eq!(
        CleanPathResult::with_line_and_column_number("Cargo.toml:30:5abc"),
        CleanPathResult {
            path: "Cargo.toml:30:5abc".into(),
            line_and_column_num: None
        }
    );

    assert_eq!(
        CleanPathResult::with_line_and_column_number("Cargo.toml[30,5]"),
        CleanPathResult {
            path: "Cargo.toml".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 30,
                column_num: Some(5)
            })
        }
    );

    assert_eq!(
        CleanPathResult::with_line_and_column_number("Cargo.toml(3,1)"),
        CleanPathResult {
            path: "Cargo.toml".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 3,
                column_num: Some(1)
            })
        }
    );

    assert_eq!(
        CleanPathResult::with_line_and_column_number("Cargo.toml\", line 100, in"),
        CleanPathResult {
            path: "Cargo.toml".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 100,
                column_num: None,
            })
        }
    );

    assert_eq!(
        CleanPathResult::with_line_and_column_number("Cargo.toml\", line 5, column 20"),
        CleanPathResult {
            path: "Cargo.toml".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 5,
                column_num: Some(20),
            })
        }
    );

    assert_eq!(
        CleanPathResult::with_line_and_column_number("Cargo.toml#L100"),
        CleanPathResult {
            path: "Cargo.toml".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 100,
                column_num: None
            })
        }
    );

    assert_eq!(
        CleanPathResult::with_line_and_column_number("Cargo.toml#L100:4"),
        CleanPathResult {
            path: "Cargo.toml".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 100,
                column_num: Some(4)
            })
        }
    );

    // Line range format :start-end (should link to start line, ignore end line)
    assert_eq!(
        CleanPathResult::with_line_and_column_number("Cargo.toml:10-50"),
        CleanPathResult {
            path: "Cargo.toml".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 10,
                column_num: None
            })
        }
    );

    assert_eq!(
        CleanPathResult::with_line_and_column_number("/path/to/file.rs:1-1000"),
        CleanPathResult {
            path: "/path/to/file.rs".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 1,
                column_num: None
            })
        }
    );

    assert_eq!(
        CleanPathResult::with_line_and_column_number("src/main.rs:100-100"),
        CleanPathResult {
            path: "src/main.rs".into(),
            line_and_column_num: Some(LineAndColumnArg {
                line_num: 100,
                column_num: None
            })
        }
    );
}

/// The single predicate every WSL UNC host check in this module goes through, so its casing and
/// host-spelling table is asserted here rather than repeated at each call site.
#[test]
fn test_is_wsl_unc_host() {
    for host in [
        "wsl$",
        "WSL$",
        "Wsl$",
        "wsl.localhost",
        "WSL.localhost",
        "Wsl.Localhost",
    ] {
        assert!(
            is_wsl_unc_host(host),
            "{host} should be recognized as a WSL UNC host"
        );
    }

    for host in [
        "",
        "wsl",
        "wsl$$",
        "wsl.localhost.example.com",
        "localhost",
        "server",
    ] {
        assert!(
            !is_wsl_unc_host(host),
            "{host} should not be recognized as a WSL UNC host"
        );
    }
}

/// Covers the prefix-kind dispatch — only UNC and verbatim UNC prefixes carry a host to check.
#[test]
fn test_is_wsl_unc_prefix() {
    fn is_wsl_unc_path_prefix(path: &str) -> bool {
        let typed_path = TypedPath::from(path);
        match typed_path.components().next() {
            Some(TypedComponent::Windows(WindowsComponent::Prefix(prefix))) => {
                is_wsl_unc_prefix(&prefix)
            }
            _ => panic!("{path} should be inferred as a Windows path with a prefix"),
        }
    }

    for path in [
        "//wsl$/Ubuntu/home",
        "//WSL.localhost/Ubuntu/home",
        r"\\wsl$\Ubuntu\home",
        r"\\?\UNC\WSL.localhost\Ubuntu\home",
    ] {
        assert!(
            is_wsl_unc_path_prefix(path),
            "{path} should be recognized as a WSL UNC prefix"
        );
    }

    for path in [
        r"\\server\share\file",
        r"\\?\UNC\server\share",
        r"\\.\COM42",
        r"\\?\pictures",
        r"C:\Users",
        r"\\?\C:\Users",
    ] {
        assert!(
            !is_wsl_unc_path_prefix(path),
            "{path} should not be recognized as a WSL UNC prefix"
        );
    }
}

/// WSL is exposed over UNC but served locally, so no spelling or casing of its host may be
/// classified as a network resource — that would make `is_path_valid` reject valid WSL file links.
#[test]
#[cfg(windows)]
fn test_is_network_resource_wsl_hosts_are_local() {
    for path in [
        r"\\WSL$\Ubuntu\home\user\file.rs",
        r"\\wsl$\Ubuntu\home\user\file.rs",
        r"\\Wsl$\Ubuntu\home\user\file.rs",
        r"\\wsl.localhost\Ubuntu\home\user\file.rs",
        r"\\WSL.localhost\Ubuntu\home\user\file.rs",
        r"\\Wsl.Localhost\Ubuntu\home\user\file.rs",
        r"\\?\UNC\wsl$\Ubuntu\home\user\file.rs",
        r"\\?\UNC\WSL$\Ubuntu\home\user\file.rs",
        r"\\?\UNC\wsl.localhost\Ubuntu\home\user\file.rs",
        r"\\?\UNC\WSL.localhost\Ubuntu\home\user\file.rs",
    ] {
        assert!(
            !is_network_resource(Path::new(path)),
            "{path} should not be treated as a network resource"
        );
    }
}

#[test]
#[cfg(windows)]
fn test_is_network_resource_non_wsl_paths() {
    for path in [
        r"\\server\share\file",
        r"\\?\UNC\server\share\file",
        r"\\wsl\Ubuntu\home",
    ] {
        assert!(
            is_network_resource(Path::new(path)),
            "{path} should be treated as a network resource"
        );
    }

    for path in [r"C:\Users\x", r"\\?\C:\Users\x", r"relative\path"] {
        assert!(
            !is_network_resource(Path::new(path)),
            "{path} should not be treated as a network resource"
        );
    }
}

#[test]
#[cfg(windows)]
fn test_msys2_exe_to_root() {
    assert_eq!(
        msys2_exe_to_root(WindowsPath::new(r"D:\Program Files\Git\usr\bin\git.exe")),
        WindowsPathBuf::from(r"D:\Program Files\Git")
    );
    assert_eq!(
        msys2_exe_to_root(WindowsPath::new(r"C:\git.exe")),
        WindowsPathBuf::from(r"C:\Program Files\Git")
    );
    assert_eq!(
        msys2_exe_to_root(WindowsPath::new(r"C:\foo\bar\baz\git.exe")),
        WindowsPathBuf::from(r"C:\Program Files\Git")
    );
    assert_eq!(
        msys2_exe_to_root(WindowsPath::new(r"C:\msys64\usr\bin\fish.exe")),
        WindowsPathBuf::from(r"C:\msys64")
    );
}

/// These tests all fail when running on UNIX.
#[test]
#[cfg(windows)]
fn test_convert_git_bash_to_windows_native_path() {
    use std::sync::LazyLock;
    static GIT_BASH_ROOT: LazyLock<WindowsPathBuf> =
        LazyLock::new(|| WindowsPathBuf::from(r"C:\Program Files\Git"));

    assert_eq!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("/c/foo/bar").to_path(),
            &GIT_BASH_ROOT
        )
        .unwrap(),
        PathBuf::from(r"C:\foo\bar")
    );
    assert_eq!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("/d/special folder").to_path(),
            &GIT_BASH_ROOT
        )
        .unwrap(),
        PathBuf::from(r"D:\special folder")
    );
    assert_eq!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("/z").to_path(),
            &GIT_BASH_ROOT
        )
        .unwrap(),
        PathBuf::from(r"Z:\")
    );
    // non-ascii isn't actually a valid drive name
    assert!(matches!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("/😊/invalid").to_path(),
            &GIT_BASH_ROOT
        ),
        Err(MSYS2PathConversionError::NotInDrive)
    ));
    assert!(matches!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("/aa/invalid").to_path(),
            &GIT_BASH_ROOT
        ),
        Err(MSYS2PathConversionError::NotInDrive)
    ));
    assert_eq!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("//wsl$/Ubuntu/home").to_path(),
            &GIT_BASH_ROOT
        )
        .unwrap(),
        PathBuf::from(r"\\wsl$\Ubuntu\home")
    );
    assert_eq!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("//WSL.localhost/Ubuntu/home").to_path(),
            &GIT_BASH_ROOT
        )
        .unwrap(),
        PathBuf::from(r"\\WSL.localhost\Ubuntu\home")
    );
    // These paths might get auto-inferred by typed-path to be Windows paths, even if they look
    // like UNIX with forward slashes. Every casing of both WSL UNC hosts must be accepted.
    for path in [
        "//wsl$/Ubuntu/home",
        "//WSL$/Ubuntu/home",
        "//Wsl$/Ubuntu/home",
        "//wsl.localhost/Ubuntu/home",
        "//WSL.localhost/Ubuntu/home",
        "//Wsl.Localhost/Ubuntu/home",
    ] {
        assert_eq!(
            convert_msys2_to_windows_native_path(&TypedPath::from(path), &GIT_BASH_ROOT)
                .unwrap_or_else(|err| panic!("{path} should be convertible, got {err:?}")),
            PathBuf::from(path.replace('/', "\\"))
        );
    }
    // A Windows-encoded UNC path to an actual remote host is still rejected.
    assert!(matches!(
        convert_msys2_to_windows_native_path(&TypedPath::from("//server/share"), &GIT_BASH_ROOT),
        Err(MSYS2PathConversionError::NonUnixPath)
    ));
    // Windows drive letters are case-insensitive, so an uppercase MSYS2 drive prefix resolves the
    // same as a lowercase one.
    assert_eq!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("/C/foo/bar").to_path(),
            &GIT_BASH_ROOT
        )
        .unwrap(),
        PathBuf::from(r"C:\foo\bar")
    );
    assert_eq!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("~/.bash_history").to_path(),
            &GIT_BASH_ROOT
        )
        .unwrap(),
        PathBuf::from(r"~\.bash_history")
    );
    // Relative paths cannot be converted.
    assert!(matches!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("some/relative/path").to_path(),
            &GIT_BASH_ROOT
        ),
        Err(MSYS2PathConversionError::PathNotAbsolute)
    ));
    assert!(matches!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_windows(r"C:\Users").to_path(),
            &GIT_BASH_ROOT
        ),
        Err(MSYS2PathConversionError::NonUnixPath)
    ));
    assert_eq!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("/").to_path(),
            &GIT_BASH_ROOT
        )
        .unwrap(),
        PathBuf::from(r"C:\Program Files\Git")
    );
    assert_eq!(
        convert_msys2_to_windows_native_path(
            &TypedPathBuf::from_unix("/usr/bin").to_path(),
            &GIT_BASH_ROOT
        )
        .unwrap(),
        PathBuf::from(r"C:\Program Files\Git\usr\bin")
    );
}

/// These tests all fail when running on UNIX.
#[test]
#[cfg(windows)]
fn test_convert_wsl_to_windows_host_path() {
    assert_eq!(
        convert_wsl_to_windows_host_path(
            &TypedPathBuf::from_unix("/mnt/c/foo/bar").to_path(),
            "Ubuntu"
        )
        .unwrap(),
        PathBuf::from(r"C:\foo\bar")
    );
    assert_eq!(
        convert_wsl_to_windows_host_path(
            &TypedPathBuf::from_unix("/mnt/e/special dir").to_path(),
            "Ubuntu"
        )
        .unwrap(),
        PathBuf::from(r"E:\special dir")
    );
    assert_eq!(
        convert_wsl_to_windows_host_path(&TypedPathBuf::from_unix("/mnt/z").to_path(), "Ubuntu")
            .unwrap(),
        PathBuf::from(r"Z:\")
    );
    // Windows drive letters are case-insensitive, so an uppercase mount resolves the same as a
    // lowercase one.
    assert_eq!(
        convert_wsl_to_windows_host_path(
            &TypedPathBuf::from_unix("/mnt/C/foo").to_path(),
            "Ubuntu"
        )
        .unwrap(),
        PathBuf::from(r"C:\foo")
    );
    assert_eq!(
        convert_wsl_to_windows_host_path(
            &TypedPathBuf::from_unix("/home/andy").to_path(),
            "Ubuntu"
        )
        .unwrap(),
        // The canonical spelling, not the caller's: lower-case host and distribution, so this
        // agrees with `canonicalize_wsl_unc_path` without being run through it.
        PathBuf::from(r"\\wsl$\ubuntu\home\andy")
    );
    assert!(matches!(
        convert_wsl_to_windows_host_path(
            &TypedPathBuf::from_unix("some/relative/path").to_path(),
            "Ubuntu"
        ),
        Err(WSLPathConversionError::PathNotAbsolute)
    ));
    assert!(matches!(
        convert_wsl_to_windows_host_path(
            &TypedPathBuf::from_unix("~/.bash_history").to_path(),
            "Ubuntu"
        ),
        Err(WSLPathConversionError::PathNotAbsolute)
    ));
    // Two letters isn't actually a valid drive.
    assert_eq!(
        convert_wsl_to_windows_host_path(
            &TypedPathBuf::from_unix("/mnt/aa/invalid_drive").to_path(),
            "Ubuntu"
        )
        .unwrap(),
        PathBuf::from(r"\\wsl$\ubuntu\mnt\aa\invalid_drive")
    );
    assert_eq!(
        convert_wsl_to_windows_host_path(
            &TypedPathBuf::from_unix("/mnt/😊/invalid_drive").to_path(),
            "Ubuntu"
        )
        .unwrap(),
        PathBuf::from(r"\\wsl$\ubuntu\mnt\😊\invalid_drive")
    );
    assert!(matches!(
        convert_wsl_to_windows_host_path(
            &TypedPathBuf::from_windows(r"C:\Users").to_path(),
            "Ubuntu"
        ),
        Err(WSLPathConversionError::NonUnixPath)
    ));
}

/// Asserts that `input` parses into the given distribution and Linux path.
fn assert_parses_wsl_unc(input: &str, distro: &str, linux_path: &str) {
    assert_eq!(
        parse_wsl_unc_path(Path::new(input)),
        Some(WslUncPath {
            distro: distro.to_string(),
            linux_path: linux_path.to_string(),
        }),
        "input: {input:?}"
    );
}

#[test]
fn test_parse_wsl_unc_path() {
    assert_parses_wsl_unc(r"\\wsl$\Ubuntu\home\user", "Ubuntu", "/home/user");
    assert_parses_wsl_unc(r"\\wsl.localhost\Ubuntu\home", "Ubuntu", "/home");
    assert_parses_wsl_unc(r"\\?\UNC\wsl$\Ubuntu\home\user", "Ubuntu", "/home/user");
    assert_parses_wsl_unc(
        r"\\?\UNC\wsl.localhost\Ubuntu\srv\repo",
        "Ubuntu",
        "/srv/repo",
    );
    assert_parses_wsl_unc("//wsl$/Ubuntu/home/user", "Ubuntu", "/home/user");
    // The host is matched case-insensitively.
    assert_parses_wsl_unc(r"\\WSL$\Ubuntu\src", "Ubuntu", "/src");
    assert_parses_wsl_unc(r"\\Wsl.LocalHost\Ubuntu\src", "Ubuntu", "/src");
    // Trailing separators are dropped.
    assert_parses_wsl_unc(r"\\wsl$\Ubuntu\home\user\", "Ubuntu", "/home/user");
    assert_parses_wsl_unc("//wsl$/Ubuntu/home/user/", "Ubuntu", "/home/user");
    // Distribution names may contain dots and dashes.
    assert_parses_wsl_unc(
        r"\\wsl.localhost\Ubuntu-24.04\home\krag",
        "Ubuntu-24.04",
        "/home/krag",
    );
    // The distribution root maps to `/`.
    assert_parses_wsl_unc(r"\\wsl$\Ubuntu", "Ubuntu", "/");
    assert_parses_wsl_unc(r"\\wsl$\Ubuntu\", "Ubuntu", "/");
    assert_parses_wsl_unc("//wsl$/archlinux", "archlinux", "/");
}

#[test]
fn test_parse_wsl_unc_path_rejects_other_paths() {
    // Non-WSL UNC share.
    assert_eq!(parse_wsl_unc_path(Path::new(r"\\server\share\dir")), None);
    // Drive-letter path.
    assert_eq!(parse_wsl_unc_path(Path::new(r"C:\Users\foo")), None);
    // Relative paths.
    assert_eq!(parse_wsl_unc_path(Path::new(r"foo\bar")), None);
    assert_eq!(parse_wsl_unc_path(Path::new("foo/bar")), None);
    // WSL host without a distribution name.
    assert_eq!(parse_wsl_unc_path(Path::new(r"\\wsl$")), None);
    assert_eq!(parse_wsl_unc_path(Path::new(r"\\wsl$\")), None);
}

#[test]
fn test_convert_windows_path_to_wsl() {
    assert_eq!(
        convert_windows_path_to_wsl(r"C:\Users\aloke\file.txt"),
        "/mnt/c/Users/aloke/file.txt"
    );
    assert_eq!(
        convert_windows_path_to_wsl(r"D:\Pictures\Screenshots\Screenshot 2025-05-14 155816.png"),
        "/mnt/d/Pictures/Screenshots/Screenshot 2025-05-14 155816.png"
    );
    // Drive letter only
    assert_eq!(convert_windows_path_to_wsl(r"C:\"), "/mnt/c");
    assert_eq!(convert_windows_path_to_wsl("C:"), "/mnt/c");
    // Uppercase drive letter should be lowercased
    assert_eq!(convert_windows_path_to_wsl(r"E:\foo"), "/mnt/e/foo");
    // Non-drive path (e.g. UNC) gets backslashes replaced
    assert_eq!(
        convert_windows_path_to_wsl(r"\\server\share\file"),
        "//server/share/file"
    );
}

#[test]
fn test_convert_windows_path_to_msys2() {
    assert_eq!(
        convert_windows_path_to_msys2(r"C:\Users\aloke\file.txt"),
        "/c/Users/aloke/file.txt"
    );
    assert_eq!(
        convert_windows_path_to_msys2(r"D:\Pictures\Screenshots\Screenshot 2025-05-14 155816.png"),
        "/d/Pictures/Screenshots/Screenshot 2025-05-14 155816.png"
    );
    // Drive letter only
    assert_eq!(convert_windows_path_to_msys2(r"C:\"), "/c");
    assert_eq!(convert_windows_path_to_msys2("C:"), "/c");
    // Uppercase drive letter should be lowercased
    assert_eq!(convert_windows_path_to_msys2(r"E:\foo"), "/e/foo");
    // Non-drive path (e.g. UNC) gets backslashes replaced
    assert_eq!(
        convert_windows_path_to_msys2(r"\\server\share\file"),
        "//server/share/file"
    );
}

#[test]
fn test_file_uri_drive_path_to_windows() {
    assert_eq!(
        file_uri_drive_path_to_windows("/E:/CLAUDE-BASE"),
        r"E:\CLAUDE-BASE"
    );
    assert_eq!(
        file_uri_drive_path_to_windows("/C:/Users/foo/bar"),
        r"C:\Users\foo\bar"
    );
    assert_eq!(file_uri_drive_path_to_windows("/E:/"), r"E:\");
    assert_eq!(file_uri_drive_path_to_windows("/E:"), "E:");
    assert_eq!(
        file_uri_drive_path_to_windows("/Users/foo/bar"),
        "/Users/foo/bar"
    );
    assert_eq!(file_uri_drive_path_to_windows("/E/notdrive"), "/E/notdrive");
    assert_eq!(file_uri_drive_path_to_windows("/E:extra"), "/E:extra");
    assert_eq!(
        file_uri_drive_path_to_windows(r"E:\CLAUDE-BASE"),
        r"E:\CLAUDE-BASE"
    );
}

#[test]
fn test_canonicalize_git_bash_path() {
    assert_eq!(
        canonicalize_git_bash_path(
            Path::new("C:")
                .join("Program Files")
                .join("Git")
                .join("bin")
                .join("bash.exe")
        ),
        Path::new("C:")
            .join("Program Files")
            .join("Git")
            .join("usr")
            .join("bin")
            .join("bash.exe")
    );
    assert_eq!(
        canonicalize_git_bash_path(
            Path::new("C:")
                .join("Windows")
                .join("system32")
                .join("bash.exe")
        ),
        Path::new("C:")
            .join("Windows")
            .join("system32")
            .join("bash.exe")
    );
}

// ── group_roots_by_common_ancestor tests ─────────────────────────────

mod group_roots_by_common_ancestor_tests {
    use std::path::PathBuf;

    use crate::path::group_roots_by_common_ancestor;

    fn pb(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn empty_input_produces_empty_grouping() {
        let grouping = group_roots_by_common_ancestor::<PathBuf>(&[]);
        assert!(grouping.roots.is_empty());
        assert!(grouping.absorbed_by_root.is_empty());
    }

    #[test]
    fn single_path_survives_with_no_absorbed() {
        let grouping = group_roots_by_common_ancestor(&[pb("/a")]);
        assert_eq!(grouping.roots, vec![pb("/a")]);
        assert!(grouping.absorbed_by_root.is_empty());
    }

    #[test]
    fn unrelated_siblings_both_survive() {
        let grouping = group_roots_by_common_ancestor(&[pb("/a"), pb("/b")]);
        assert_eq!(grouping.roots, vec![pb("/a"), pb("/b")]);
        assert!(grouping.absorbed_by_root.is_empty());
    }

    #[test]
    fn descendant_absorbed_into_ancestor() {
        // Ancestor listed first.
        let grouping = group_roots_by_common_ancestor(&[pb("/a"), pb("/a/b")]);
        assert_eq!(grouping.roots, vec![pb("/a")]);
        assert_eq!(grouping.absorbed_by_root.len(), 1);
        assert_eq!(grouping.absorbed_by_root[&pb("/a")], vec![pb("/a/b")]);
    }

    #[test]
    fn descendant_first_still_absorbed() {
        // Descendant listed first, ancestor second; survivor is still the
        // ancestor and its input order is preserved.
        let grouping = group_roots_by_common_ancestor(&[pb("/a/b"), pb("/a")]);
        assert_eq!(grouping.roots, vec![pb("/a")]);
        assert_eq!(grouping.absorbed_by_root[&pb("/a")], vec![pb("/a/b")]);
    }

    #[test]
    fn three_deep_chain_collapses_to_root() {
        // Descendant order in input is preserved in the absorbed list.
        let grouping = group_roots_by_common_ancestor(&[pb("/a/b/c"), pb("/a/b"), pb("/a")]);
        assert_eq!(grouping.roots, vec![pb("/a")]);
        assert_eq!(
            grouping.absorbed_by_root[&pb("/a")],
            vec![pb("/a/b/c"), pb("/a/b")]
        );
    }

    #[test]
    fn mixed_groups_absorb_independently() {
        let grouping =
            group_roots_by_common_ancestor(&[pb("/a"), pb("/x"), pb("/a/b"), pb("/x/y")]);
        assert_eq!(grouping.roots, vec![pb("/a"), pb("/x")]);
        assert_eq!(grouping.absorbed_by_root[&pb("/a")], vec![pb("/a/b")]);
        assert_eq!(grouping.absorbed_by_root[&pb("/x")], vec![pb("/x/y")]);
    }

    #[test]
    fn same_prefix_different_component_name_both_survive() {
        // /foo/a is NOT an ancestor of /foo/abc (component-aware match).
        let grouping = group_roots_by_common_ancestor(&[pb("/foo/a"), pb("/foo/abc")]);
        assert_eq!(grouping.roots, vec![pb("/foo/a"), pb("/foo/abc")]);
        assert!(grouping.absorbed_by_root.is_empty());
    }

    #[test]
    fn duplicate_inputs_collapse_to_single_survivor() {
        let grouping = group_roots_by_common_ancestor(&[pb("/a"), pb("/a")]);
        assert_eq!(grouping.roots, vec![pb("/a")]);
        assert!(grouping.absorbed_by_root.is_empty());
    }

    #[test]
    fn surviving_root_order_matches_input_order() {
        // Insert a descendant between two surviving ancestors; the survivors
        // should appear in their original input order even though processing
        // sorted by component count.
        let grouping = group_roots_by_common_ancestor(&[pb("/b"), pb("/a/x"), pb("/a"), pb("/c")]);
        assert_eq!(grouping.roots, vec![pb("/b"), pb("/a"), pb("/c")]);
        assert_eq!(grouping.absorbed_by_root[&pb("/a")], vec![pb("/a/x")]);
    }

    #[test]
    fn descendant_absorbed_by_closest_ancestor_not_furthest() {
        // Both /a and /a/b are surviving ancestors of /a/b/c... wait, /a/b is
        // itself absorbed into /a. So /a/b/c should be absorbed into /a as
        // well (the only surviving ancestor).
        let grouping = group_roots_by_common_ancestor(&[pb("/a"), pb("/a/b"), pb("/a/b/c")]);
        assert_eq!(grouping.roots, vec![pb("/a")]);
        assert_eq!(
            grouping.absorbed_by_root[&pb("/a")],
            vec![pb("/a/b"), pb("/a/b/c")]
        );
    }
}

// ── One spelling for a WSL directory (T6.2) ──────────────────────────────

/// The whole point of the function: every spelling Windows accepts for one directory has to come
/// back as one string, because a `HashMap<PathBuf, _>` cannot compare hosts case-insensitively
/// the way `is_wsl_unc_host` does. The list is the one measured on the fork — all seven of these
/// open the same directory, and `GetFinalPathNameByHandleW` returns seven different answers.
#[test]
fn every_spelling_of_one_wsl_directory_canonicalizes_to_the_same_string() {
    let canonical = PathBuf::from(r"\\wsl$\ubuntu\home\effatha\git\warp");

    for path in [
        r"\\wsl$\Ubuntu\home\effatha\git\warp",
        r"\\WSL$\Ubuntu\home\effatha\git\warp",
        r"\\wsl$\ubuntu\home\effatha\git\warp",
        r"\\wsl$\UBUNTU\home\effatha\git\warp",
        r"\\wsl.localhost\Ubuntu\home\effatha\git\warp",
        r"\\WSL.LOCALHOST\Ubuntu\home\effatha\git\warp",
        r"\\?\UNC\WSL$\Ubuntu\home\effatha\git\warp",
        r"\\?\UNC\wsl.localhost\ubuntu\home\effatha\git\warp",
        "//wsl$/Ubuntu/home/effatha/git/warp",
    ] {
        assert_eq!(
            canonicalize_wsl_unc_path(Path::new(path)),
            Some(canonical.clone()),
            "{path} should canonicalize to {}",
            canonical.display()
        );
    }
}

/// Linux file names are case-sensitive: `\\wsl$\Ubuntu\home\effatha\git\WARP` is
/// `ERROR_FILE_NOT_FOUND` on the machine where `warp` exists. Folding case past the distribution
/// would name a different file, or none, so the fold has to stop exactly there.
#[test]
fn the_linux_path_keeps_its_case() {
    assert_eq!(
        canonicalize_wsl_unc_path(Path::new(r"\\WSL$\Ubuntu\home\Effatha\Git\WARP")),
        Some(PathBuf::from(r"\\wsl$\ubuntu\home\Effatha\Git\WARP"))
    );
}

/// A distribution root has no Linux path to append; it must not come back with a trailing
/// separator, or it stops being a prefix of its own children by string comparison.
#[test]
fn a_distribution_root_canonicalizes_without_a_trailing_separator() {
    for path in [
        r"\\wsl$\Ubuntu",
        r"\\WSL$\Ubuntu\",
        r"\\?\UNC\wsl.localhost\Ubuntu",
    ] {
        assert_eq!(
            canonicalize_wsl_unc_path(Path::new(path)),
            Some(PathBuf::from(r"\\wsl$\ubuntu")),
            "{path} should canonicalize to the bare distribution root"
        );
    }
}

/// Everything that is not a WSL UNC path is somebody else's normal form. Returning `None` rather
/// than a guess is what lets the call sites say `.unwrap_or(original)`.
#[test]
fn non_wsl_paths_are_left_to_their_own_canonicalization() {
    for path in [
        r"C:\dev\warp",
        r"\\?\C:\dev\warp",
        r"\\server\share\file",
        r"\\?\UNC\server\share\file",
        r"\\wsl\Ubuntu\home",
        "/home/effatha/git/warp",
        r"relative\path",
    ] {
        assert_eq!(
            canonicalize_wsl_unc_path(Path::new(path)),
            None,
            "{path} is not a WSL UNC path"
        );
    }
}

/// Canonicalizing an already-canonical path must be a no-op, or repeated normalization walks the
/// key away from itself.
#[test]
fn canonicalization_is_idempotent() {
    for path in [r"\\wsl$\ubuntu\home\effatha", r"\\wsl$\ubuntu"] {
        let once = canonicalize_wsl_unc_path(Path::new(path)).expect("is a WSL UNC path");
        assert_eq!(once, PathBuf::from(path));
        assert_eq!(canonicalize_wsl_unc_path(&once), Some(once.clone()));
    }
}

/// The spelling chosen as canonical has to be one the rest of the module still recognizes —
/// otherwise `is_network_resource` would start calling this fork's own paths remote.
#[test]
fn the_canonical_spelling_is_still_recognized_as_wsl() {
    assert!(is_wsl_unc_host(CANONICAL_WSL_UNC_HOST));

    let canonical =
        canonicalize_wsl_unc_path(Path::new(r"\\WSL.localhost\Ubuntu\home\effatha")).unwrap();
    assert_eq!(
        parse_wsl_unc_path(&canonical),
        Some(WslUncPath {
            distro: "ubuntu".to_string(),
            linux_path: "/home/effatha".to_string(),
        })
    );
}

/// The verbatim form is the one that most needs to go: nobody typed it, and `cmd.exe` refuses it
/// ("UNC paths are not supported") where it accepts the canonical spelling of the same directory.
#[test]
fn a_verbatim_wsl_path_is_never_what_a_person_is_shown() {
    assert_eq!(
        user_friendly_path(r"\\?\UNC\WSL$\Ubuntu\home\effatha\git\warp", None),
        r"\\wsl$\ubuntu\home\effatha\git\warp"
    );
}

/// The display seam folds the same spellings the key does, so a directory reads one way wherever
/// it appears rather than three ways at once.
#[test]
fn one_wsl_directory_is_displayed_one_way() {
    let home = Some(r"C:\Users\effatha");
    let displayed: Vec<_> = [
        r"\\wsl$\Ubuntu\home\effatha",
        r"\\WSL$\Ubuntu\home\effatha",
        r"\\wsl.localhost\ubuntu\home\effatha",
        r"\\?\UNC\WSL.localhost\Ubuntu\home\effatha",
    ]
    .iter()
    .map(|path| user_friendly_path(path, home).into_owned())
    .collect();

    assert_eq!(
        displayed,
        vec![r"\\wsl$\ubuntu\home\effatha".to_string(); 4]
    );
}

/// The home-directory abbreviation still has to work, and still has to leave ordinary paths
/// alone — the WSL fold sits in front of it, not instead of it.
#[test]
fn the_home_abbreviation_is_unaffected_by_the_wsl_fold() {
    assert_eq!(
        user_friendly_path("/Users/blue/warp", Some("/Users/blue")),
        "~/warp"
    );
    assert_eq!(
        user_friendly_path(r"\\server\share\file", None),
        r"\\server\share\file"
    );

    // A WSL path whose home directory matches is abbreviated too, after the fold rather than
    // instead of it. Windows-only because the abbreviation only accepts a `\` separator when
    // compiled for Windows — see the TODO on `abbreviate_home_dir`, which this fork has not
    // touched.
    #[cfg(windows)]
    assert_eq!(
        user_friendly_path(
            r"\\?\UNC\WSL$\Ubuntu\home\effatha\git",
            Some(r"\\wsl$\ubuntu\home\effatha")
        ),
        r"~\git"
    );
}

/// Everything that is not a WSL path has to come back borrowed, untouched, and without having
/// been through a path parser — this runs on every path Warp displays, once per block among
/// other places.
#[test]
fn non_wsl_paths_skip_the_fold_entirely() {
    for path in [
        "",
        "/",
        "/home/effatha/git/warp",
        r"C:\dev\warp",
        r"\\server\share",
        r"\\?\C:\dev\warp",
        "relative/path",
    ] {
        assert_eq!(
            display_spelling_of_wsl_path(path),
            None,
            "{path} should not be re-spelled for display"
        );
    }
}

// ── Dragging a WSL file into a WSL session (T6.2) ────────────────────────

/// Explorer shows WSL files as `\\wsl$\Ubuntu\home\…`. Dropping one into a session on that same
/// distribution has to insert the path the distribution knows the file by. The generic
/// conversion only swaps separators, and `//wsl$/Ubuntu/home/…` is not a path inside WSL —
/// Linux collapses the leading `//` and looks for `/wsl$/Ubuntu/…`, which does not exist.
#[test]
fn a_file_dropped_from_this_distribution_converts_to_its_linux_path() {
    for path in [
        r"\\wsl$\Ubuntu\home\effatha\git\warp\README.md",
        r"\\WSL$\Ubuntu\home\effatha\git\warp\README.md",
        r"\\wsl.localhost\ubuntu\home\effatha\git\warp\README.md",
        r"\\?\UNC\WSL$\Ubuntu\home\effatha\git\warp\README.md",
    ] {
        assert_eq!(
            convert_windows_path_to_wsl_in_distro(path, "Ubuntu"),
            "/home/effatha/git/warp/README.md",
            "{path} should convert to the path Ubuntu knows it by"
        );
    }

    // The distribution in the path and the session's distribution are compared the way
    // `wsl.exe --distribution` compares them, and the way `git.rs` already did.
    assert_eq!(
        convert_windows_path_to_wsl_in_distro(r"\\wsl$\UBUNTU\home\effatha", "ubuntu"),
        "/home/effatha"
    );
}

/// A drive path is what this conversion was always for, and it has to keep working — most drops
/// are still ordinary Windows files.
#[test]
fn a_file_dropped_from_a_windows_drive_still_converts_to_mnt() {
    assert_eq!(
        convert_windows_path_to_wsl_in_distro(r"C:\Users\aloke\file.txt", "Ubuntu"),
        "/mnt/c/Users/aloke/file.txt"
    );
    assert_eq!(
        convert_windows_path_to_wsl_in_distro(r"E:\foo", "Ubuntu"),
        "/mnt/e/foo"
    );
}

/// A file in another distribution has no path from inside this one. Falling back to the generic
/// conversion hands back what was dropped rather than inventing a path that resolves to the
/// wrong file, or to nothing.
#[test]
fn a_file_dropped_from_another_distribution_is_not_rewritten() {
    assert_eq!(
        convert_windows_path_to_wsl_in_distro(r"\\wsl$\Debian\home\user\file.txt", "Ubuntu"),
        "//wsl$/Debian/home/user/file.txt"
    );
    // Nor is an ordinary network share, which is reachable from neither side by a Linux path.
    assert_eq!(
        convert_windows_path_to_wsl_in_distro(r"\\server\share\file", "Ubuntu"),
        "//server/share/file"
    );
}

/// The distribution root itself, which `parse_wsl_unc_path` reports as `/`.
#[test]
fn the_distribution_root_converts_to_the_filesystem_root() {
    assert_eq!(
        convert_windows_path_to_wsl_in_distro(r"\\wsl$\Ubuntu", "Ubuntu"),
        "/"
    );
}
