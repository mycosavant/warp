use std::path::{Path, PathBuf};
use std::sync::Arc;

use warp_core::SessionId;
use warp_util::local_or_remote_path::LocalOrRemotePath;
use warp_util::remote_path::RemotePath;
use warp_util::standardized_path::StandardizedPath;
use warpui::{AppContext, Entity, ModelContext, ModelHandle};

use super::{Session, SessionType, Sessions};
use crate::ai_assistant::execution_context::{
    WarpAiExecutionContext, execution_context_for_session,
};
use crate::terminal::ShellLaunchData;
use crate::terminal::model::session::SessionsEvent;
use crate::terminal::model_events::{ModelEvent, ModelEventDispatcher};
use crate::terminal::shell::ShellType;

pub struct ActiveSession {
    model_event_dispatcher: ModelHandle<ModelEventDispatcher>,
    sessions: ModelHandle<Sessions>,

    /// The current working directory of the terminal session.
    current_working_directory: Option<String>,
}

impl ActiveSession {
    pub fn new(
        sessions: ModelHandle<Sessions>,
        model_event_dispatcher: ModelHandle<ModelEventDispatcher>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&model_event_dispatcher, move |me, _, event, ctx| {
            let new_pwd = match event {
                ModelEvent::BlockMetadataReceived(e) => Some(
                    e.block_metadata
                        .current_working_directory()
                        .map(|cwd| cwd.to_owned()),
                ),
                ModelEvent::BlockWorkingDirectoryUpdated(e) => Some(
                    e.block_metadata
                        .current_working_directory()
                        .map(|cwd| cwd.to_owned()),
                ),
                _ => None,
            };
            if let Some(new_pwd) = new_pwd
                && me.current_working_directory != new_pwd
            {
                me.current_working_directory = new_pwd;
                ctx.emit(ActiveSessionEvent::UpdatedPwd);
            }
        });

        ctx.subscribe_to_model(&sessions, |me, _, event, ctx| {
            if let SessionsEvent::SessionBootstrapped(bootstrap_event) = event
                && Some(bootstrap_event.session_id)
                    == me.model_event_dispatcher.as_ref(ctx).active_session_id()
            {
                ctx.emit(ActiveSessionEvent::Bootstrapped);
            }
        });

        Self {
            sessions,
            model_event_dispatcher,
            current_working_directory: None,
        }
    }

    pub fn session(&self, app: &AppContext) -> Option<Arc<Session>> {
        self.session_id(app)
            .and_then(|session_id| self.sessions.as_ref(app).get(session_id))
    }

    pub fn session_id(&self, app: &AppContext) -> Option<SessionId> {
        self.model_event_dispatcher.as_ref(app).active_session_id()
    }

    pub fn session_type(&self, app: &AppContext) -> Option<SessionType> {
        self.session(app).map(|session| session.session_type())
    }

    pub fn shell_type(&self, app: &AppContext) -> Option<ShellType> {
        self.session(app)
            .as_ref()
            .map(|session| session.shell().shell_type())
    }

    pub fn shell_launch_data(&self, app: &AppContext) -> Option<ShellLaunchData> {
        self.session(app)
            .as_ref()
            .and_then(|session| session.launch_data().cloned())
    }

    pub fn current_working_directory(&self) -> Option<&String> {
        self.current_working_directory.as_ref()
    }

    /// Returns a session-aware path for `path`.
    ///
    /// Local session paths are canonicalized to match git-detected repository paths on
    /// case-insensitive filesystems. Remote session paths are standardized and tagged with
    /// the connected host ID.
    ///
    /// **A WSL session is `SessionType::Local` and still yields a remote path**,
    /// which is the one case where the session's own type is not the answer.
    /// `determine_session_type` decides by hostname equality and WSL2 inherits
    /// the Windows machine name, so a WSL session can never be
    /// `WarpifiedRemote` and `set_remote_host_id` silently no-ops on it — a
    /// server attached by `remote wsl connect` publishes a host id that the
    /// session then discards. Routing here rather than reclassifying the
    /// session is deliberate: `SessionType::Local` also drives path conversion,
    /// agent execution context, command corrections and chips, and flipping it
    /// would change all of them at once. See T16.
    pub fn location_for_path(&self, path: &str, app: &AppContext) -> Option<LocalOrRemotePath> {
        match self.session_type(app) {
            Some(SessionType::WarpifiedRemote {
                host_id: Some(host_id),
            }) => StandardizedPath::try_new(path)
                .ok()
                .map(|path| LocalOrRemotePath::Remote(RemotePath::new(host_id, path))),
            Some(SessionType::WarpifiedRemote { host_id: None }) => None,
            Some(SessionType::Local) | None => {
                if let Some(remote) = self.wsl_location_for_path(path, app) {
                    return Some(remote);
                }
                let path =
                    dunce::canonicalize(Path::new(path)).unwrap_or_else(|_| PathBuf::from(path));
                Some(LocalOrRemotePath::Local(path))
            }
        }
    }

    /// A remote path for a WSL session that has a remote server attached, or
    /// `None` for every other session.
    ///
    /// Gated on the server actually being connected, so this changes nothing
    /// until `warpctrl remote wsl connect` has succeeded: before that a WSL
    /// pane keeps the local UNC path it has always had, rather than losing its
    /// file tree to a host that cannot answer.
    ///
    /// `path` is passed through unconverted, because it is the *Linux* path the
    /// shell reported and the Linux path is what the server on the far side
    /// wants. The UNC rewrite that `normalize_cwd` would apply is exactly what
    /// this is avoiding.
    #[cfg(not(target_family = "wasm"))]
    fn wsl_location_for_path(&self, path: &str, app: &AppContext) -> Option<LocalOrRemotePath> {
        use warpui::SingletonEntity as _;

        use crate::remote_server::manager::RemoteServerManager;

        let session = self.session(app)?;
        session.wsl_name()?;

        let host_id = RemoteServerManager::as_ref(app)
            .host_for_connected_session(session.id())?
            .clone();

        StandardizedPath::try_new(path)
            .ok()
            .map(|path| LocalOrRemotePath::Remote(RemotePath::new(host_id, path)))
    }

    #[cfg(target_family = "wasm")]
    fn wsl_location_for_path(&self, _path: &str, _app: &AppContext) -> Option<LocalOrRemotePath> {
        None
    }

    pub fn current_working_directory_location(
        &self,
        app: &AppContext,
    ) -> Option<LocalOrRemotePath> {
        let cwd = self.current_working_directory()?;
        self.location_for_path(cwd.as_str(), app)
    }

    /// Returns the `WarpAiExecutionContext` for the active session.
    pub fn ai_execution_environment(&self, app: &AppContext) -> Option<WarpAiExecutionContext> {
        self.session(app)
            .as_ref()
            .map(execution_context_for_session)
    }
}

pub enum ActiveSessionEvent {
    /// The active session's working directory changed.
    UpdatedPwd,
    /// The active session finished bootstrapping.
    Bootstrapped,
}

impl Entity for ActiveSession {
    type Event = ActiveSessionEvent;
}
