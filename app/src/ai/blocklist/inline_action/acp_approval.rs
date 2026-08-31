//! The panel's answer to an ACP permission request (`.fork/TASKS.md`, T14.16).
//!
//! # Why this exists
//!
//! Until now the fork's only answerable surfaces were `warpctrl` and the T12
//! console, which is the odd shape T14.9 recorded: **the surface a person is
//! least likely to be looking at was the one that could be clicked.** The panel
//! printed the request as prose and offered nothing. Measured over a seven-turn
//! session, about thirty-five requests were answered by copying an id and a
//! 64-character digest into a shell.
//!
//! T14.8 removed most of that cost by printing the exact command that answers
//! each request, and shelved this on the argument that a button over a request
//! with no yes is a greyed-out button. Two things then argued it back: the
//! `other`-kind population turned out to be one agent's convention rather than a
//! protocol fact, and T14.11 found that an *unattended* answerer is not
//! available at all — so a present person answering cheaply is not a
//! convenience, it is the only mechanism there is.
//!
//! # What it may show, and what it may not
//!
//! `acp_permission`'s rule governs here exactly as it governs the flag: **an
//! option may only be selected by a surface capable of showing what that option
//! declares.** This surface shows the agent, the title, the verbatim tool input,
//! where the call said it would act, and every option the agent offered. So it
//! may offer the single-shot yes — and nothing else. The always-variants are not
//! rendered at all, because a button that sets a session policy would be
//! authorising something never shown.
//!
//! **The yes is gated per entry on [`ParkedRequest::approve_selects`]**, which is
//! frozen when the request parks. That is T14.6's bug, and it is worth naming
//! because this is a third surface to make it in: the console once drew a *Yes*
//! from a per-device fact with no per-entry check, so a phone showed a button on
//! rows the handler would always reject. An entry Warp will not approve shows the
//! reason instead of a button.
//!
//! # Why the action carries the id it was drawn with
//!
//! A click answers **the request that was on screen when it was read**, never
//! "whatever is pending now". The registry is a live map: a request can be
//! answered from `warpctrl` or the console, or its turn can end, and another can
//! park in the same place between a person reading this and clicking it. Looking
//! the request up again at click time would answer the newcomer with a decision
//! made about its predecessor — which is precisely the stale-answer hazard the
//! control plane's digest exists to prevent. Capturing the id at render time
//! gets the same property structurally, because here the surface that displayed
//! it is the surface that answers it.
//!
//! # Two taps for yes, one for no
//!
//! The console's asymmetry, for the console's reason: saying yes runs something
//! on this machine, and saying no can only ever make less happen. A misclick on
//! *No* costs the agent a retry; a misclick on *Yes* costs whatever it asked for.

use ui_components::{Component as _, Options as _, button};
use warp_core::ui::appearance::Appearance;
use warpui::elements::{Container, CrossAxisAlignment, Element, Flex, ParentElement, Text};
use warpui::{AppContext, Entity, SingletonEntity as _, TypedActionView, View, ViewContext};

use crate::ai::acp_agent::registry::{self, ParkedRequest};

pub enum AcpApprovalViewEvent {
    /// A person answered. The block uses this to drop the view rather than
    /// waiting for the next output update, so the question stops being on
    /// screen the moment it stops being a question.
    Answered,
}

#[derive(Clone, Debug)]
pub enum AcpApprovalViewAction {
    /// First tap on *Yes*. Arms that one request and nothing else.
    ArmAllow {
        approval_id: String,
    },
    /// Second tap. Answers the request named here, not whatever is pending.
    Allow {
        approval_id: String,
    },
    Deny {
        approval_id: String,
    },
}

pub struct AcpApprovalView {
    /// Which conversation's questions this view draws. The panel shows one
    /// conversation; `registry::waiting_for` is the only lookup that answers
    /// "is this one mine", because the other two ids on a parked request
    /// describe the agent's session and Warp's chosen directory.
    conversation_id: String,
    /// The approval id whose *Yes* is armed, if any. Holding the id rather than
    /// a bool means arming one request and then clicking after a different one
    /// arrived does nothing.
    armed: Option<String>,
    allow: button::Button,
    deny: button::Button,
}

impl AcpApprovalView {
    pub fn new(conversation_id: String) -> Self {
        Self {
            conversation_id,
            armed: None,
            allow: button::Button::default(),
            deny: button::Button::default(),
        }
    }

    /// The request this view is currently about, if any.
    ///
    /// The oldest, because requests are answered in the order they were asked
    /// and an agent that asks twice should not have its second question jump the
    /// first. `waiting_for` preserves registry order.
    fn current(&self) -> Option<ParkedRequest> {
        registry::waiting_for(&self.conversation_id)
            .into_iter()
            .next()
    }

    /// Whether a second tap on this id should answer it.
    ///
    /// Extracted so the rule can be tested without a view context, because it is
    /// the rule that makes the two-step yes a safety property rather than a
    /// flourish: arming request A and clicking after B has replaced it must do
    /// nothing at all. A `bool` here would answer B with a decision made about A.
    fn armed_for(&self, approval_id: &str) -> bool {
        self.armed.as_deref() == Some(approval_id)
    }

    /// One labelled line of disclosure, soft-wrapped.
    ///
    /// **Wrapping is not cosmetic here.** The first live run rendered the
    /// refusal sentence with `Text::new_inline`, copied from a neighbouring
    /// inline view, and the sentence ran off the right edge of the panel — so
    /// the one field whose entire purpose is to explain *why there is no yes*
    /// was the field a person could not finish reading. `Text::new_inline`'s own
    /// doc calls itself deprecated for this reason ("all usages have not been
    /// audited"); `Text::new` soft-wraps and is the one to copy.
    ///
    /// Label and value share one text element rather than sitting in a row,
    /// because a row would need its value child to be flexible to wrap at all,
    /// and a line that cannot wrap is the bug this replaces.
    fn line(label: &str, value: &str, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let text = if label.is_empty() {
            value.to_owned()
        } else {
            format!("{label}  {value}")
        };
        Container::new(
            Text::new(
                text,
                appearance.monospace_font_family(),
                appearance.monospace_font_size() - 1.0,
            )
            .with_color(theme.sub_text_color(theme.surface_1()).into())
            .finish(),
        )
        .with_vertical_padding(2.)
        .finish()
    }
}

impl Entity for AcpApprovalView {
    type Event = AcpApprovalViewEvent;
}

impl TypedActionView for AcpApprovalView {
    type Action = AcpApprovalViewAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            AcpApprovalViewAction::ArmAllow { approval_id } => {
                self.armed = Some(approval_id.clone());
            }
            AcpApprovalViewAction::Allow { approval_id } => {
                // Re-checked here and not only at render, because the arming
                // state is what a second tap is answering *about*. An armed id
                // that is no longer the armed id is a tap on a question that
                // has been replaced since the first tap.
                if self.armed_for(approval_id) {
                    registry::answer(
                        approval_id,
                        registry::Decision::Allow,
                        registry::Surface::Panel,
                    );
                    self.armed = None;
                    ctx.emit(AcpApprovalViewEvent::Answered);
                }
            }
            AcpApprovalViewAction::Deny { approval_id } => {
                registry::answer(
                    approval_id,
                    registry::Decision::Deny,
                    registry::Surface::Panel,
                );
                self.armed = None;
                ctx.emit(AcpApprovalViewEvent::Answered);
            }
        }
        ctx.notify();
    }
}

impl View for AcpApprovalView {
    fn ui_name() -> &'static str {
        "AcpApprovalView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let Some(parked) = self.current() else {
            // Nothing waiting: an empty column rather than a placeholder, so a
            // conversation with no question looks exactly as it did before.
            return Flex::column().finish();
        };
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        column.add_child(
            Text::new_inline(
                format!("{} wants permission", parked.agent),
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
            )
            .with_color(theme.main_text_color(theme.surface_1()).into())
            .finish(),
        );

        if let Some(title) = &parked.title {
            column.add_child(Self::line("", title, app));
        }
        if let Some(input) = &parked.tool_input {
            column.add_child(Self::line("the call", input, app));
        }
        // Never falls back to the session directory: that is Warp's own choice of
        // where to run the agent, not the agent's claim about where this call
        // acts, and drawing one as the other is the vagueness T14.6 measured.
        column.add_child(Self::line(
            "acts on",
            &if parked.acts_on.is_empty() {
                "not stated by the agent".to_owned()
            } else {
                parked.acts_on.join(", ")
            },
            app,
        ));
        if !parked.options_offered.is_empty() {
            column.add_child(Self::line(
                "offered",
                &parked.options_offered.join(", "),
                app,
            ));
        }

        let mut buttons = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);

        match &parked.approve_selects {
            Some(_) => {
                let armed = self.armed.as_deref() == Some(parked.approval_id.as_str());
                let id = parked.approval_id.clone();
                buttons.add_child(
                    Container::new(
                        self.allow.render(
                            appearance,
                            button::Params {
                                content: button::Content::Label(
                                    if armed {
                                        "tap again to allow"
                                    } else {
                                        "Yes, once"
                                    }
                                    .into(),
                                ),
                                theme: &button::themes::Primary,
                                options: button::Options {
                                    size: button::Size::Small,
                                    on_click: Some(Box::new(move |ctx, _, _| {
                                        ctx.dispatch_typed_action(if armed {
                                            AcpApprovalViewAction::Allow {
                                                approval_id: id.clone(),
                                            }
                                        } else {
                                            AcpApprovalViewAction::ArmAllow {
                                                approval_id: id.clone(),
                                            }
                                        });
                                    })),
                                    ..button::Options::default(appearance)
                                },
                            },
                        ),
                    )
                    .with_padding_right(8.)
                    .finish(),
                );
            }
            None => {
                // The reason, not a disabled button. A greyed control says "not
                // now"; this is "not by Warp, and here is why" — and the
                // sentence is the only thing that tells a person whether they
                // are looking at a setting or a fault.
                column.add_child(Self::line(
                    "no yes",
                    parked
                        .approve_refused_because
                        .as_deref()
                        .unwrap_or("Warp will not say yes to this request, and did not say why."),
                    app,
                ));
            }
        }

        let deny_id = parked.approval_id.clone();
        buttons.add_child(self.deny.render(
            appearance,
            button::Params {
                content: button::Content::Label("No".into()),
                theme: &button::themes::Secondary,
                options: button::Options {
                    size: button::Size::Small,
                    on_click: Some(Box::new(move |ctx, _, _| {
                        ctx.dispatch_typed_action(AcpApprovalViewAction::Deny {
                            approval_id: deny_id.clone(),
                        });
                    })),
                    ..button::Options::default(appearance)
                },
            },
        ));

        column.add_child(
            Container::new(buttons.finish())
                .with_padding_top(6.)
                .finish(),
        );

        Container::new(column.finish())
            .with_horizontal_padding(12.)
            .with_vertical_padding(10.)
            .finish()
    }
}

#[cfg(test)]
#[path = "acp_approval_tests.rs"]
mod tests;
