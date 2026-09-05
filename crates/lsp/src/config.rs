use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
#[cfg(not(target_arch = "wasm32"))]
use command::r#async::Command;
use lsp_types::{
    ClientCapabilities, ClientInfo, DidChangeWatchedFilesClientCapabilities, GotoCapability,
    HoverClientCapabilities, InitializeParams, MarkupKind, PublishDiagnosticsClientCapabilities,
    TextDocumentClientCapabilities, TextDocumentSyncClientCapabilities, Uri,
    WindowClientCapabilities, WorkDoneProgressParams, WorkspaceClientCapabilities, WorkspaceFolder,
};

use crate::supported_servers::LSPServerType;

/// Result of resolving an LSP server command, including the command and init params.
#[cfg(not(target_arch = "wasm32"))]
pub struct ResolvedLspCommand {
    pub command: Command,
    pub params: InitializeParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
    Rust,
    Go,
    Python,
    TypeScript,
    TypeScriptReact,
    JavaScript,
    JavaScriptReact,
    C,
    Cpp,
}

impl LanguageId {
    pub fn from_path(path: &Path) -> Option<Self> {
        let extn = path.extension()?;
        match extn.to_str()? {
            "rs" => Some(Self::Rust),
            "go" => Some(Self::Go),
            "py" => Some(Self::Python),
            "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::TypeScriptReact),
            "js" | "mjs" | "cjs" => Some(Self::JavaScript),
            "jsx" => Some(Self::JavaScriptReact),
            "c" | "C" => Some(Self::C),
            "cc" | "cpp" | "cxx" => Some(Self::Cpp),
            // NOTE: `.h` files are ambiguous (could be C or C++). We map them to Cpp
            // because clangd defaults to C++ for `.h` files anyway. When a
            // compile_commands.json is present, clangd will use the correct language
            // regardless of the languageId we send.
            "h" | "H" | "hh" | "hpp" | "hxx" => Some(Self::Cpp),
            _ => None,
        }
    }

    /// Returns the language identifier as used by LSP.
    /// See: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocumentItem
    pub(crate) fn lsp_language_identifier(&self) -> &'static str {
        match self {
            LanguageId::Rust => "rust",
            LanguageId::Go => "go",
            LanguageId::Python => "python",
            LanguageId::TypeScript => "typescript",
            LanguageId::TypeScriptReact => "typescriptreact",
            LanguageId::JavaScript => "javascript",
            LanguageId::JavaScriptReact => "javascriptreact",
            LanguageId::C => "c",
            LanguageId::Cpp => "cpp",
        }
    }

    /// For now we assume a 1:1 language -> LSP server type. This might change in the future as we support more configurabilities.
    pub fn server_type(&self) -> LSPServerType {
        match self {
            LanguageId::Rust => LSPServerType::RustAnalyzer,
            LanguageId::Go => LSPServerType::GoPls,
            LanguageId::Python => LSPServerType::Pyright,
            LanguageId::TypeScript
            | LanguageId::TypeScriptReact
            | LanguageId::JavaScript
            | LanguageId::JavaScriptReact => LSPServerType::TypeScriptLanguageServer,
            LanguageId::C | LanguageId::Cpp => LSPServerType::Clangd,
        }
    }
}

/// Configuration for spawning an LSP server process.
#[derive(Clone)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub struct LspServerConfig {
    server_type: LSPServerType,
    initial_workspace: PathBuf,
    /// The local PATH variable set when starting the server. This is needed when the app is started
    /// without a shell based parent process.
    /// TODO(kevin): This might not be sufficient for all cases (e.g. user might remove LSP from PATH).
    path_env_var: Option<String>,
    client_name: String,
    /// Shared HTTP client used for LSP installation checks and downloads.
    client: Arc<http_client::Client>,
    /// Optional path relative to the LSP log namespace for server stderr output.
    log_relative_path: Option<PathBuf>,
    /// The WSL distribution to run the server inside, when the workspace is a
    /// `\\wsl$\<distro>\...` path on Windows. `None` is upstream's behaviour:
    /// the server is a process of this machine, reading the workspace directly.
    /// See [`UriMapper`] for what else changes when this is set.
    wsl_distro: Option<String>,
}

impl fmt::Debug for LspServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LspServerConfig")
            .field("server_type", &self.server_type)
            .field("initial_workspace", &self.initial_workspace)
            .field("path_env_var", &self.path_env_var)
            .field("client_name", &self.client_name)
            .field("log_relative_path", &self.log_relative_path)
            .field("wsl_distro", &self.wsl_distro)
            .finish()
    }
}

impl LspServerConfig {
    pub fn new(
        server_type: LSPServerType,
        initial_workspace: PathBuf,
        path_env_var: Option<String>,
        client_name: String,
        client: Arc<http_client::Client>,
    ) -> Self {
        Self {
            server_type,
            initial_workspace,
            path_env_var,
            client_name,
            client,
            log_relative_path: None,
            wsl_distro: None,
        }
    }

    /// Runs the server inside `distro` instead of on this machine. The
    /// workspace stays a Windows path; the spawn wraps the binary in
    /// `wsl.exe` and the URI seam translates (`.fork/docs/wsl.md`, item 3).
    pub fn with_wsl_distro(mut self, distro: String) -> Self {
        self.wsl_distro = Some(distro);
        self
    }

    /// The distribution this server runs inside, if it is not a process of
    /// this machine.
    pub fn wsl_distro(&self) -> Option<&str> {
        self.wsl_distro.as_deref()
    }

    /// How this server's paths are spelled on the wire.
    pub fn uri_mapper(&self) -> UriMapper {
        match &self.wsl_distro {
            Some(distro) => UriMapper::WslDistro(distro.clone()),
            None => UriMapper::Local,
        }
    }

    /// Sets the relative log path for this server's stderr output.
    pub fn with_log_relative_path(mut self, log_relative_path: PathBuf) -> Self {
        self.log_relative_path = Some(log_relative_path);
        self
    }
    /// Returns the relative log path if configured.
    pub fn log_relative_path(&self) -> Option<&PathBuf> {
        self.log_relative_path.as_ref()
    }

    /// Returns the initial workspace path.
    pub fn initial_workspace(&self) -> &Path {
        &self.initial_workspace
    }

    pub(crate) fn server_name(&self) -> String {
        self.server_type.binary_name().to_string()
    }

    pub(crate) fn languages(&self) -> Vec<LanguageId> {
        self.server_type.languages()
    }

    /// Creates the command and init params for the LSP server.
    ///
    /// PATH takes precedence over custom installations. If the binary is available
    /// and working on PATH, we use that. Otherwise, we fall back to our custom installation.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) async fn command_and_params(self) -> Result<ResolvedLspCommand> {
        let mapper = self.uri_mapper();
        // PATH takes precedence - only use custom installation if not working on PATH
        let executor = crate::CommandBuilder::new(self.path_env_var.clone())
            .in_wsl_distro(self.wsl_distro.clone());
        let is_working_on_path = self
            .server_type
            .is_working_on_path(&executor, self.client.clone())
            .await;
        // A server that runs inside a WSL distribution has to come from that
        // distribution's PATH: what Warp downloads into its data directory is
        // a Windows executable, and `wsl.exe` cannot run one.
        let custom_binary_config = if is_working_on_path || self.wsl_distro.is_some() {
            // Binary works on PATH, don't use custom installation
            None
        } else {
            // Not working on PATH, check for custom installation
            self.server_type
                .find_installed_binary_config(executor.path_env_var())
                .await
        };

        // Bail early with a clear error instead of attempting to spawn a
        // binary that doesn't exist (which would fail with a confusing
        // "No such file or directory" OS error).
        if !is_working_on_path && custom_binary_config.is_none() {
            match &self.wsl_distro {
                Some(distro) => bail!(
                    "{} is not on the PATH inside the WSL distribution {distro}. Install it \
                     there; a copy on Windows cannot serve a workspace inside the distribution",
                    self.server_type.binary_name()
                ),
                None => bail!(
                    "{} is not installed. Binary was not found on PATH and no custom installation exists",
                    self.server_type.binary_name()
                ),
            }
        }

        let mut command = self
            .server_type
            .create_command(custom_binary_config.clone(), &executor);

        // Set the working directory to the workspace root. This is required for
        // LSP servers like rust-analyzer to properly discover the project structure.
        // For a server inside a WSL distribution this is the `\\wsl$\...` root,
        // which `wsl.exe` maps to the Linux directory it stands for -- measured
        // 2026-09-05, `pwd` inside the child answered `/home/...`.
        command.current_dir(&self.initial_workspace);

        log::info!(
            "LSP {} starting with custom_binary_config: {:?}",
            self.server_type.binary_name(),
            custom_binary_config
        );

        let params = default_init_params(&self.initial_workspace, self.client_name, &mapper)?;

        Ok(ResolvedLspCommand { command, params })
    }

    pub(crate) fn server_type(&self) -> LSPServerType {
        self.server_type
    }
}

/// How a server's paths are spelled on the wire.
///
/// `Local` is upstream's mapping: `url::Url::from_file_path` one way and a
/// percent-decode the other, for a server that is a process of this machine.
///
/// `WslDistro` is the fork's, for a server running inside a WSL distribution
/// while Warp keys its workspace on the `\\wsl$\<distro>\...` spelling every
/// other map in the app uses (`.fork/docs/wsl.md`, item 3). Out:
/// `\\wsl$\ubuntu\home\x.rs` becomes `file:///home/x.rs`. In: every
/// `file:///...` the server answers with comes back under the same prefix,
/// including paths outside the workspace such as a toolchain's `core` source,
/// because a server inside the distribution can only ever name files inside
/// it. Measured 2026-09-05: a Linux `rust-analyzer` spawned by a Windows
/// process through `wsl.exe` answered a definition with these URIs in 3.4 s
/// where the Windows binary over the redirector took 25 s on the same crate.
///
/// This is a mapping rather than a second call to [`path_to_lsp_uri`] because
/// that function cannot spell a Linux path on Windows at all:
/// `Path::is_absolute` is false without a drive or UNC prefix, and
/// `url::Url::from_file_path` refuses on that alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriMapper {
    Local,
    WslDistro(String),
}

impl UriMapper {
    pub(crate) fn to_uri(&self, path: &Path) -> Result<Uri> {
        match self {
            Self::Local => path_to_lsp_uri(path),
            Self::WslDistro(distro) => wsl_path_to_lsp_uri(path, distro),
        }
    }

    pub(crate) fn to_path(&self, uri: &Uri) -> Result<PathBuf> {
        match self {
            Self::Local => lsp_uri_to_path(uri),
            Self::WslDistro(distro) => lsp_uri_to_wsl_path(uri, distro),
        }
    }
}

/// `\\wsl$\<distro>\a\b.rs` (any of the spellings `parse_wsl_unc_path`
/// accepts) to `file:///a/b.rs`, for a server running inside `distro`.
fn wsl_path_to_lsp_uri(path: &Path, distro: &str) -> Result<Uri> {
    let parsed = warp_util::path::parse_wsl_unc_path(path)
        .ok_or_else(|| anyhow!("Path is not inside a WSL distribution: {}", path.display()))?;
    // Distribution names are case-insensitive on the redirector, which is why
    // the canonical spelling lower-cases them; compare the same way.
    if !parsed.distro.eq_ignore_ascii_case(distro) {
        bail!(
            "Path is inside WSL distribution {} but this server runs inside {distro}: {}",
            parsed.distro,
            path.display()
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // `set_path` percent-encodes what a path segment cannot carry, which
        // is the part of `Url::from_file_path` this needs and the only part
        // it can do on this side of the boundary.
        let mut url = url::Url::parse("file:///").expect("a constant file URL parses");
        url.set_path(&parsed.linux_path);
        // Same bracket rule as `path_to_lsp_uri`, for the same servers.
        let uri_str = url.as_str().replace('[', "%5B").replace(']', "%5D");
        uri_str.parse::<Uri>().map_err(anyhow::Error::from)
    }

    #[cfg(target_arch = "wasm32")]
    {
        format!("file://{}", parsed.linux_path)
            .parse::<Uri>()
            .map_err(anyhow::Error::from)
    }
}

/// `file:///a/b.rs` from a server inside `distro` to `\\wsl$\<distro>\a\b.rs`,
/// in the lower-cased spelling `warp_util::path::canonicalize_wsl_unc_path`
/// folds every other spelling to, so the result is a key the rest of the app
/// already holds rather than a second name for the same file.
fn lsp_uri_to_wsl_path(uri: &Uri, distro: &str) -> Result<PathBuf> {
    if uri.scheme().map(|s| s.as_str()) != Some("file") {
        bail!("Invalid file URI: {}", uri.as_str());
    }
    if let Some(authority) = uri.authority() {
        let host = authority.host().as_str();
        if !host.is_empty() && host != "localhost" {
            bail!(
                "URI names a host, not the distribution the server runs inside: {}",
                uri.as_str()
            );
        }
    }

    let decoded = uri
        .path()
        .as_estr()
        .decode()
        .into_string()
        .map_err(|e| anyhow!("Invalid UTF-8 in URI path: {e}"))?;
    let linux_path: &str = decoded.as_ref();
    if !linux_path.starts_with('/') {
        bail!("URI path is not absolute: {}", uri.as_str());
    }

    let mut spelled = format!(r"\\wsl$\{}", distro.to_ascii_lowercase());
    // The distribution root is the whole path already; a translated `/` would
    // leave a trailing separator on it.
    if linux_path != "/" {
        spelled.push_str(&linux_path.replace('/', r"\"));
    }
    Ok(PathBuf::from(spelled))
}

pub(crate) fn path_to_lsp_uri(path: &Path) -> Result<Uri> {
    if !path.is_absolute() {
        return Err(anyhow::anyhow!("Path must be absolute: {}", path.display()));
    }

    // url::Url::from_file_path handles percent-encoding internally but is not
    // available on WASM. LSP is not supported on WASM either, so the fallback
    // is a simple string concatenation.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let url = url::Url::from_file_path(path).map_err(|()| {
            anyhow::anyhow!("Failed to convert path to file URI: {}", path.display())
        })?;

        // The url crate doesn't encode brackets, but LSP requires them to be
        // percent-encoded (e.g. Next.js [slug].tsx routes).
        let uri_str = url.as_str().replace('[', "%5B").replace(']', "%5D");

        uri_str.parse::<Uri>().map_err(anyhow::Error::from)
    }

    #[cfg(target_arch = "wasm32")]
    {
        let path_str = path.to_string_lossy();
        let uri_string = format!("file://{path_str}");
        uri_string.parse::<Uri>().map_err(anyhow::Error::from)
    }
}

pub(crate) fn lsp_uri_to_path(uri: &Uri) -> Result<PathBuf> {
    // Validate this is a file URI
    let scheme = uri.scheme().map(|s| s.as_str());
    if scheme != Some("file") {
        return Err(anyhow::anyhow!("Invalid file URI: {}", uri.as_str()));
    }

    // Decode percent-encoded characters (e.g., %40 -> @)
    // This is necessary because LSP servers return URL-encoded paths
    let decoded_path = uri
        .path()
        .as_estr()
        .decode()
        .into_string()
        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in URI path: {e}"))?;

    let mut path_str: &str = decoded_path.as_ref();

    // Windows URIs are formatted like: file:///C:/path/to/file
    // The path component is `/C:/path/to/file`, strip the leading slash.
    if cfg!(windows) {
        path_str = path_str.strip_prefix('/').unwrap_or(path_str);
        return Ok(PathBuf::from(path_str.replace('/', "\\")));
    }

    Ok(PathBuf::from(path_str))
}

fn path_to_workspace_folder(path: &Path, mapper: &UriMapper) -> Result<WorkspaceFolder> {
    mapper.to_uri(path).map(|url| WorkspaceFolder {
        uri: url,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
    })
}

fn default_client_capabilities() -> ClientCapabilities {
    ClientCapabilities {
        workspace: Some(WorkspaceClientCapabilities {
            did_change_watched_files: Option::from(DidChangeWatchedFilesClientCapabilities {
                dynamic_registration: Some(true),
                relative_pattern_support: Some(true),
            }),
            ..Default::default()
        }),
        window: Some(WindowClientCapabilities {
            work_done_progress: Some(true),
            ..Default::default()
        }),
        text_document: Some(TextDocumentClientCapabilities {
            synchronization: Some(TextDocumentSyncClientCapabilities {
                dynamic_registration: Some(true),
                will_save: Some(false),
                will_save_wait_until: Some(false),
                did_save: Some(true),
            }),
            definition: Some(GotoCapability {
                dynamic_registration: Some(false),
                link_support: Some(true),
            }),
            hover: Some(HoverClientCapabilities {
                dynamic_registration: Some(false),
                // Request Markdown content from the LSP for hover responses.
                // This enables proper syntax highlighting in hover tooltips.
                content_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
            }),
            publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                version_support: Some(true),
                related_information: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn default_init_params(
    workspace_uri: &Path,
    client_name: String,
    mapper: &UriMapper,
) -> Result<InitializeParams> {
    let workspace_folder = path_to_workspace_folder(workspace_uri, mapper)?;

    Ok(InitializeParams {
        process_id: Some(std::process::id()),
        capabilities: default_client_capabilities(),
        workspace_folders: Some(vec![workspace_folder]),
        client_info: Some(ClientInfo {
            name: client_name,
            version: option_env!("GIT_RELEASE_TAG").map(|s| s.to_string()),
        }),
        locale: None,
        work_done_progress_params: WorkDoneProgressParams::default(),
        ..Default::default()
    })
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
