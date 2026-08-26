use warp_core::features::FeatureFlag;
use warp_core::send_telemetry_from_ctx;
use warp_core::ui::appearance::Appearance;
use warp_errors::report_error;
use warpui::keymap::Keystroke;
use warpui::{AppContext, EntityId, SingletonEntity, ViewContext};

use crate::TelemetryEvent;
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::ai::blocklist::agent_view::{
    AgentViewEntryBlock, AgentViewEntryBlockEvent, AgentViewEntryBlockParams, AgentViewEntryOrigin,
    AutoTriggerBehavior, DismissalStrategy, ENTER_OR_EXIT_CONFIRMATION_WINDOW, EnterAgentViewError,
    EphemeralMessage,
};
use crate::ai::blocklist::history_model::CloudConversationData;
use crate::global_resource_handles::GlobalResourceHandlesProvider;
use crate::persistence::ModelEvent;
use crate::search::slash_command_menu::static_commands::StaticCommand;
use crate::server::telemetry::TelemetryAgentViewEntryOrigin;
use crate::terminal::TerminalView;
use crate::terminal::input::message_bar::{Message, MessageItem};
use crate::terminal::model::rich_content::RichContentType;
use crate::terminal::view::load_ai_conversation::{
    RestoreConversationEntryBehavior, RestoredAIConversation,
};
use crate::terminal::view::{
    AgentViewEntryMetadata, Event, RichContentInsertionPosition, RichContentMetadata,
};
use crate::view_components::DismissibleToast;
use crate::workspace::ToastStack;

pub const ENTER_AGAIN_TO_SEND_MESSAGE_ID: &str = "enter_again_to_send";

/// Where `warpctrl agent reveal` should put a conversation.
///
/// Named here rather than taking the protocol enum so the terminal view does
/// not have to know what a local-control request looks like; the handler maps
/// one to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalControlRevealTarget {
    /// Split it off next to the pane it was spawned from, keeping both on
    /// screen. The default, because it is the only one of the three that adds
    /// a surface rather than taking one over.
    Split,
    /// Open it in a new tab.
    Tab,
    /// Swap it into the targeted pane, which is what clicking the pill in the
    /// orchestration bar does. The pane it replaces is not closed — the swap
    /// is reversible — but the caller loses sight of what was there.
    Swap,
}

impl TerminalView {
    /// Sends `prompt` to the agent, on behalf of `warpctrl agent prompt`.
    ///
    /// Returns the conversation the prompt went to, or `None` when the agent is
    /// monitoring a long-running command and cannot take one. An orchestrator
    /// needs the id back: having started three conversations it has no other way
    /// to tell which is which, and `agent.list` cannot say which of three new
    /// entries belongs to the call that just returned.
    ///
    /// Deliberately *not* built on `enter_agent_view`, which is what the
    /// keybinding path uses: that swallows the id and reports failure by
    /// raising a toast, which is the right behaviour for a person at the
    /// keyboard and useless to a caller over a socket.
    pub(crate) fn start_agent_conversation_from_local_control(
        &mut self,
        prompt: String,
        conversation_id: Option<AIConversationId>,
        ctx: &mut ViewContext<Self>,
    ) -> Option<AIConversationId> {
        // The same guard `enter_agent_view_for_new_conversation` applies, minus
        // the toast. Continuing an existing conversation is exempt, exactly as
        // it is there — the guard is about *starting* one.
        if conversation_id.is_none()
            && !self
                .ai_context_model
                .as_ref(ctx)
                .can_start_new_conversation()
        {
            return None;
        }

        // `Cli` and not `Input`, and the difference is the whole feature.
        // `try_enter_agent_view` asks the origin whether to submit the prompt or
        // merely stage it in the composer, and `Input` answers
        // `AutoTriggerBehavior::InAgentView` — submit only if the pane was
        // *already* in agent view. Driving this from `warpctrl` it never is, so
        // the first version put the prompt on screen and left it there, which
        // looked exactly like success until the screenshot was read.
        // `Cli => AutoTriggerBehavior::Always` is the case this already had a
        // name for.
        let origin = AgentViewEntryOrigin::Cli;
        match self.try_enter_agent_view(Some(prompt), origin.clone(), conversation_id, ctx) {
            Ok(id) => {
                self.redetermine_global_focus(ctx);
                Some(id)
            }
            Err(error) => {
                report_error!(
                    anyhow::Error::new(error)
                        .context("Failed to enter agent view for local control"),
                    extra: { "origin" => ?origin }
                );
                None
            }
        }
    }

    /// The conversation this terminal's input would act on, for the
    /// prompt-submitting slash commands.
    ///
    /// `/compact` has to apply to the conversation in front of you, not to a
    /// new one, so `slash.run` resolves it here rather than letting the agent
    /// view pick.
    pub(crate) fn selected_conversation_for_local_control(
        &self,
        ctx: &AppContext,
    ) -> Option<AIConversationId> {
        self.ai_context_model
            .as_ref(ctx)
            .selected_conversation_id(ctx)
    }

    /// Whether a slash command would run in this terminal right now.
    pub(crate) fn slash_command_is_available_for_local_control(
        &self,
        command: &StaticCommand,
        ctx: &AppContext,
    ) -> bool {
        self.input()
            .as_ref(ctx)
            .slash_command_is_available_for_local_control(command, ctx)
    }

    /// Runs a slash command in this terminal's input, on behalf of
    /// `warpctrl slash run`. See [`Input::run_slash_command_from_local_control`].
    pub(crate) fn run_slash_command_from_local_control(
        &mut self,
        command: &StaticCommand,
        argument: Option<String>,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        self.input().update(ctx, |input, ctx| {
            input.run_slash_command_from_local_control(command, argument, ctx)
        })
    }

    /// Stops the turn a conversation is running, on behalf of
    /// `warpctrl agent cancel`.
    ///
    /// Emits the same event the "Stop agent" item in a child pill's menu does,
    /// rather than calling [`TerminalView::stop_local_agent_conversation`]
    /// directly, because the pane group's handler is where the choice between
    /// stopping a local turn and cancelling a cloud task is made — and where
    /// the fallback lives for a conversation whose owning view is in another
    /// tab. Reaching past it would work for the common case and quietly do
    /// nothing for the rest.
    pub(crate) fn stop_agent_conversation_from_local_control(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.emit(Event::StopAgentConversation { conversation_id });
    }

    /// Presses one key on the CLI agent running in this pane, on behalf of
    /// `warpctrl agent approve` (T11.5).
    ///
    /// **This is a keystroke, not a verdict.** Warp has no channel that tells a
    /// CLI agent "approved" — the agent is a process drawing a prompt on a PTY,
    /// and the only thing a person sitting here could do is press a key. So that
    /// is what this does, and the caller is told which key so the result never
    /// reads as more than it was.
    ///
    /// Goes through [`TerminalView::write_user_bytes_to_pty`] rather than
    /// `write_to_pty` for the check it carries: a block under Warp's *own* agent
    /// control refuses the write and returns `false`, which is reported as a
    /// failure rather than swallowed. A caller told "denied" about a keystroke
    /// that never left the process is the silent failure this whole phase exists
    /// to catch.
    pub(crate) fn press_key_for_local_control(
        &mut self,
        bytes: &'static [u8],
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        self.write_user_bytes_to_pty(bytes, ctx)
    }

    /// Puts a conversation on screen, on behalf of `warpctrl agent reveal`.
    ///
    /// All three targets already exist as events, because the 3-dot menu on a
    /// child pill offers all three. The events are group-scoped, so the caller
    /// is responsible for having found a view in the tab that holds the
    /// conversation — see `agent::agent_reveal`, which refuses rather than
    /// emitting into the wrong group and reporting a success that never
    /// happened.
    pub(crate) fn reveal_agent_conversation_from_local_control(
        &mut self,
        conversation_id: AIConversationId,
        target: LocalControlRevealTarget,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.emit(match target {
            LocalControlRevealTarget::Split => Event::OpenChildAgentInNewPane { conversation_id },
            LocalControlRevealTarget::Tab => Event::OpenChildAgentInNewTab { conversation_id },
            LocalControlRevealTarget::Swap => Event::RevealChildAgent { conversation_id },
        });
    }

    pub fn enter_agent_view(
        &mut self,
        initial_prompt: Option<String>,
        conversation_id: Option<AIConversationId>,
        origin: AgentViewEntryOrigin,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(id) = conversation_id {
            self.enter_agent_view_for_conversation(initial_prompt, origin, id, ctx);
        } else {
            self.enter_agent_view_for_new_conversation(initial_prompt, origin, ctx);
        }
    }

    pub fn enter_agent_view_for_new_conversation(
        &mut self,
        initial_prompt: Option<String>,
        origin: AgentViewEntryOrigin,
        ctx: &mut ViewContext<Self>,
    ) {
        // Don't allow starting a new conversation while the agent is in control. 3p cloud
        // viewers enter agent view to wrap an existing run's content and are not starting a
        // new conversation, so they are exempt from this guard.
        if !matches!(&origin, AgentViewEntryOrigin::ThirdPartyCloudAgent)
            && !self
                .ai_context_model
                .as_ref(ctx)
                .can_start_new_conversation()
        {
            let window_id = ctx.window_id();
            ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                toast_stack.add_ephemeral_toast(
                    DismissibleToast::error(
                        "Cannot start a new conversation while agent is monitoring a command."
                            .to_string(),
                    ),
                    window_id,
                    ctx,
                );
            });
            return;
        }

        if let Err(e) = self.try_enter_agent_view(initial_prompt, origin.clone(), None, ctx) {
            report_error!(
                anyhow::Error::new(e).context("Failed to enter agent view for new conversation"),
                extra: { "origin" => ?origin }
            );
            self.show_error_toast(e.to_string(), ctx);
        }
        self.redetermine_global_focus(ctx);
    }

    // Enters the agent view for a restored CLI agent transcript, setting the title using the
    // restored CLI conversation metadata if we have it.
    pub(crate) fn enter_agent_view_for_restored_cli_agent(
        &mut self,
        fallback_title: String,
        ctx: &mut ViewContext<Self>,
    ) -> Option<AIConversationId> {
        let origin = AgentViewEntryOrigin::ThirdPartyCloudAgent;

        match self.try_enter_agent_view(None, origin.clone(), None, ctx) {
            Ok(conversation_id) => {
                let title = fallback_title.trim();
                if !title.is_empty() {
                    BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, _| {
                        if let Some(conversation) = history.conversation_mut(&conversation_id) {
                            conversation.set_fallback_display_title(title.to_owned());
                        }
                    });
                }
                self.redetermine_global_focus(ctx);
                Some(conversation_id)
            }
            Err(e) => {
                report_error!(
                    anyhow::Error::new(e).context("Failed to enter agent view for restored CLI agent"),
                    extra: { "origin" => ?origin }
                );
                self.show_error_toast(e.to_string(), ctx);
                self.redetermine_global_focus(ctx);
                None
            }
        }
    }

    pub fn enter_agent_view_for_conversation(
        &mut self,
        initial_prompt: Option<String>,
        origin: AgentViewEntryOrigin,
        conversation_id: AIConversationId,
        ctx: &mut ViewContext<Self>,
    ) {
        let history_model = BlocklistAIHistoryModel::handle(ctx);
        let (in_memory_conversation, is_live) = {
            let history_model_ref = history_model.as_ref(ctx);
            let in_memory_conversation = history_model_ref.conversation(&conversation_id).cloned();
            let is_live = in_memory_conversation.is_some()
                && history_model_ref
                    .all_live_conversations_for_terminal_surface(self.view_id)
                    .any(|conversation| conversation.id() == conversation_id);
            (in_memory_conversation, is_live)
        };

        if is_live {
            if let Err(e) = self.try_enter_agent_view(
                initial_prompt.clone(),
                origin.clone(),
                Some(conversation_id),
                ctx,
            ) {
                report_error!(
                    anyhow::Error::new(e).context("Failed to enter agent view for existing conversation"),
                    extra: { "conversation_id" => ?conversation_id, "origin" => ?origin }
                );
                self.show_error_toast(e.to_string(), ctx);
            }
        } else if let Some(conversation) = in_memory_conversation {
            self.restore_conversation_after_view_creation(
                RestoredAIConversation::new(conversation),
                false,
                RestoreConversationEntryBehavior::PreserveAgentViewState,
                ctx,
            );
            if let Err(e) = self.try_enter_agent_view(
                initial_prompt.clone(),
                origin.clone(),
                Some(conversation_id),
                ctx,
            ) {
                report_error!(
                    anyhow::Error::new(e)
                        .context("Failed to enter agent view for restored in-memory conversation"),
                    extra: { "conversation_id" => ?conversation_id, "origin" => ?origin }
                );
                self.show_error_toast(e.to_string(), ctx);
            }
        } else {
            let conversation_id_copy = conversation_id;
            let future = history_model
                .as_ref(ctx)
                .load_conversation_data(conversation_id_copy, ctx);
            ctx.spawn(future, move |me, conversation, ctx| {
                let Some(conversation) = conversation else {
                    me.show_error_toast(
                        format!("Failed to load conversation with id: {conversation_id}"),
                        ctx,
                    );
                    return;
                };
                // For Oz conversations, restore data and then re-enter agent view (the
                // conversation will be in memory after restoration).
                // For CLI agent conversations, restore the block snapshot only. Because we
                // don't update the in-memory model in this case, attempting to re-enter agent
                // view will trigger an infinite loop of fetching and loading conversation data
                // from the server.
                #[allow(clippy::type_complexity)]
                let on_restored: Box<
                    dyn FnOnce(&mut Self, &mut ViewContext<Self>),
                > = if matches!(&conversation, CloudConversationData::Oz(_)) {
                    Box::new(move |me, ctx| {
                        me.enter_agent_view_for_conversation(
                            initial_prompt,
                            origin,
                            conversation_id,
                            ctx,
                        );
                    })
                } else {
                    if !FeatureFlag::AgentHarness.is_enabled() {
                        log::warn!("AgentHarness flag is disabled; ignoring CLI agent conversation {conversation_id}");
                        return;
                    }
                    Box::new(|_, _| {})
                };
                let is_local = BlocklistAIHistoryModel::handle(ctx)
                    .as_ref(ctx)
                    .get_conversation_metadata(&conversation_id)
                    .is_some_and(|m| m.has_local_data);
                me.restore_conversation_and_directory_context(
                    conversation,
                    false,
                    RestoreConversationEntryBehavior::PreserveAgentViewState,
                    is_local,
                    on_restored,
                    ctx,
                );
            });
        }
    }

    pub(super) fn try_enter_agent_view(
        &mut self,
        initial_prompt: Option<String>,
        origin: AgentViewEntryOrigin,
        conversation_id: Option<AIConversationId>,
        ctx: &mut ViewContext<Self>,
    ) -> Result<AIConversationId, EnterAgentViewError> {
        // Capture pending context block IDs before entering agent view.
        let pending_attached_blocks = self
            .ai_context_model
            .as_ref(ctx)
            .pending_context_block_ids()
            .clone();
        let was_in_agent_view_already = self
            .agent_view_controller
            .as_ref(ctx)
            .agent_view_state()
            .is_fullscreen();

        let conversation_id = self.agent_view_controller.update(ctx, |controller, ctx| {
            controller.try_enter_agent_view(conversation_id, origin.clone(), ctx)
        })?;

        // Associate pending context blocks with the new conversation so they remain
        // visible in the agent view. This must happen after the conversation is created
        // but before any re-filtering of pending context block IDs occurs.
        if !pending_attached_blocks.is_empty() {
            let attached_blocks = self
                .model
                .lock()
                .block_list_mut()
                .associate_blocks_with_conversation(
                    pending_attached_blocks.iter(),
                    conversation_id,
                );

            // Persist the updated visibility for each modified block
            if let Some(sender) = GlobalResourceHandlesProvider::as_ref(ctx)
                .get()
                .model_event_sender
                .as_ref()
            {
                for (block_id, agent_view_visibility) in attached_blocks {
                    if let Err(e) = sender.send(ModelEvent::UpdateBlockAgentViewVisibility {
                        block_id: block_id.to_string(),
                        agent_view_visibility: agent_view_visibility.into(),
                    }) {
                        report_error!(
                            anyhow::Error::new(e)
                                .context("Error sending UpdateBlockAgentViewVisibility event")
                        );
                    }
                }
            }
        }

        let mut did_auto_trigger_request = false;
        // Show ephemeral message when entering agent view via input with a prompt
        if let Some(initial_prompt) = initial_prompt {
            let should_auto_submit = match origin.should_autotrigger_request() {
                AutoTriggerBehavior::Always => true,
                AutoTriggerBehavior::InAgentView => was_in_agent_view_already,
                AutoTriggerBehavior::Never => false,
            };
            if should_auto_submit {
                // Clear the "enter again to send" ephemeral message if it's currently showing
                self.ephemeral_message_model.update(ctx, |model, ctx| {
                    if model
                        .current_message()
                        .and_then(|msg| msg.id())
                        .is_some_and(|id| id == ENTER_AGAIN_TO_SEND_MESSAGE_ID)
                    {
                        model.clear_message(ctx);
                    }
                });

                self.ai_controller.update(ctx, |controller, ctx| {
                    controller.send_user_query_in_conversation(
                        initial_prompt,
                        conversation_id,
                        None,
                        ctx,
                    );
                });
                did_auto_trigger_request = true;
            } else {
                let appearance = Appearance::handle(ctx).as_ref(ctx);
                let message = Message::new(vec![
                    MessageItem::keystroke(Keystroke {
                        key: "enter".to_owned(),
                        ..Default::default()
                    }),
                    MessageItem::text("again to send to agent"),
                ])
                .with_text_color(appearance.theme().ansi_fg_magenta());
                self.ephemeral_message_model.update(ctx, |model, ctx| {
                    // Keep this explicit (instead of relying on the default message duration) so
                    // "enter again to send" stays aligned with the broader confirmation cadence.
                    model.show_ephemeral_message(
                        EphemeralMessage::new(
                            message,
                            DismissalStrategy::Timer(ENTER_OR_EXIT_CONFIRMATION_WINDOW),
                        )
                        .with_id(ENTER_AGAIN_TO_SEND_MESSAGE_ID),
                        ctx,
                    );
                });

                self.input.update(ctx, |input, ctx| {
                    input.replace_buffer_content(&initial_prompt, ctx);
                });
            }
        }

        send_telemetry_from_ctx!(
            TelemetryEvent::AgentViewEntered {
                origin: TelemetryAgentViewEntryOrigin::from(origin),
                did_auto_trigger_request,
            },
            ctx
        );

        // Mark all AgentViewEntry rich content as dirty so their heights get
        // re-measured. When the agent view is active, AgentViewEntryBlock renders
        // as Empty (0 height). When exiting, we need to force a re-layout so the
        // block's actual height is restored. The dirty item processing happens
        // before viewport iteration, so this works even for 0-height items at
        // the prefix of the blocklist.
        let mut model = self.model.lock();
        self.mark_all_rich_content_items_dirty_where(&mut model, |metadata| {
            matches!(metadata, RichContentMetadata::AgentViewEntry(_))
        });
        drop(model);

        Ok(conversation_id)
    }

    pub(super) fn insert_agent_view_entry_block(
        &mut self,
        params: AgentViewEntryBlockParams,
        position: RichContentInsertionPosition,
        ctx: &mut ViewContext<Self>,
    ) {
        if BlocklistAIHistoryModel::as_ref(ctx)
            .conversation(&params.conversation_id)
            .is_some_and(|conversation| conversation.is_entirely_passive())
        {
            return;
        }
        let conversation_id = params.conversation_id;
        let origin = params.origin.clone();
        let agent_view_block =
            ctx.add_typed_action_view(|ctx| AgentViewEntryBlock::new(params, ctx));
        ctx.subscribe_to_view(&agent_view_block, |me, _, event, ctx| match event {
            AgentViewEntryBlockEvent::EnterAgentView { conversation_id } => me
                .enter_agent_view_for_conversation(
                    None,
                    AgentViewEntryOrigin::AgentViewBlock,
                    *conversation_id,
                    ctx,
                ),
            AgentViewEntryBlockEvent::OpenConversationContextMenu {
                conversation_id,
                agent_view_entry_block_id,
                position,
            } => me.open_agent_view_entry_context_menu(
                *conversation_id,
                *agent_view_entry_block_id,
                *position,
                ctx,
            ),
            AgentViewEntryBlockEvent::ForkConversation { conversation_id } => {
                me.fork_ai_conversation(*conversation_id, None, ctx);
            }
        });
        self.insert_rich_content(
            Some(RichContentType::EnterAgentView),
            agent_view_block,
            Some(RichContentMetadata::AgentViewEntry(
                AgentViewEntryMetadata {
                    conversation_id,
                    origin,
                },
            )),
            position,
            ctx,
        );
    }

    /// Retags the rich content view with the given id so it renders under `conversation_id`'s
    /// agent view. Updates both the local `rich_content_views` entry and the block list so
    /// `should_hide_for_agent_view_state` picks up the new association.
    pub(super) fn set_rich_content_agent_view_conversation_id(
        &mut self,
        rich_content_view_id: EntityId,
        conversation_id: AIConversationId,
    ) {
        let Some(rich_content) = self
            .rich_content_views
            .iter_mut()
            .find(|rich_content| rich_content.view_id() == rich_content_view_id)
        else {
            return;
        };

        rich_content.set_agent_view_conversation_id(Some(conversation_id));
        self.model
            .lock()
            .block_list_mut()
            .update_agent_view_conversation_id_for_rich_content(
                rich_content_view_id,
                Some(conversation_id),
            );
    }
}
