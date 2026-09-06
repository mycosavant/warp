//! The block `/remote-control` puts in the pane (2026-09-05, the fork's
//! remote control; `app/src/local_control/remote_control.rs`).
//!
//! Claude Code's `/remote-control` prints a QR code in the terminal; this is
//! the same thing in Warp's own pane, as a rich-content block the way the
//! plugin instructions are. It holds a pairing code, so what it shows is a
//! secret spendable once and dead in two minutes, and it says so beside the
//! code rather than in a doc. Closing the block does not end remote control:
//! the code stays spendable until it expires, and a phone that already
//! scanned stays paired until *Stop sharing* in the footer cuts it off.
use pathfinder_geometry::vector::vec2f;
use warpui::clipboard::ClipboardContent;
use warpui::color::ColorU;
use warpui::elements::{
    Border, ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    Empty, Expanded, Flex, MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, Radius, SizeConstraintCondition, SizeConstraintSwitch, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::{
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::appearance::Appearance;
use crate::drive::sharing::qr_code::{QUIET_ZONE_MODULES, QrMatrix, qr_matrix_for_url};
use crate::ui_components::icons::Icon;
use crate::view_components::DismissibleToast;
use crate::view_components::action_button::{ActionButton, ButtonSize, NakedTheme};
use crate::workspace::ToastStack;

/// How wide the QR is drawn, in points. Big enough for a phone camera across
/// a desk; the sharing dialog uses 160 and that scanned.
const QR_SIZE: f32 = 176.;

/// Below this width the text goes under the QR instead of beside it. The QR,
/// the gap and the padding take about 240 points, and a text column narrower
/// than the remaining 240 wraps a URL to five lines and a sentence to a word
/// a line. Measured in a 300-pixel pane on 2026-09-05
/// (`.fork/runs/remote-control-2026-09-05/rc-before.png`): drawn as a row,
/// the text ran off the right edge, because it sat in an `Align`, which hands
/// its child unbounded width.
const STACK_BELOW_WIDTH: f32 = 480.;

pub(crate) struct RemoteControlBlock {
    url: String,
    /// Where a phone that has not installed the console's certificate
    /// authority yet fetches it, in the clear (T19). Once per phone.
    ca_url: Option<String>,
    matrix: Option<QrMatrix>,
    expires_at: String,
    actions: Vec<String>,
    close_button: ViewHandle<ActionButton>,
    copy_button: ViewHandle<ActionButton>,
    should_hide: bool,
}

impl RemoteControlBlock {
    pub(crate) fn new(
        result: &::local_control::protocol::PairingResult,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let close_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("", NakedTheme)
                .with_icon(Icon::X)
                .with_size(ButtonSize::XSmall)
                .with_tooltip("Hide this block (the code stays spendable until it expires)")
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(RemoteControlBlockAction::Close);
                })
        });
        let copy_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Copy link", NakedTheme)
                .with_icon(Icon::Copy)
                .with_size(ButtonSize::XSmall)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(RemoteControlBlockAction::CopyLink);
                })
        });
        Self {
            matrix: qr_matrix_for_url(&result.url).ok(),
            url: result.url.clone(),
            ca_url: result.ca_url.clone(),
            expires_at: result.expires_at.format("%H:%M:%S UTC").to_string(),
            actions: result.actions.clone(),
            close_button,
            copy_button,
            should_hide: false,
        }
    }

    /// The matrix as a grid of squares on a white card, the way the sharing
    /// dialog draws its QR. White around it on purpose: a QR needs its quiet
    /// zone light, and the pane behind this is usually dark.
    fn render_qr(&self, matrix: &QrMatrix) -> Box<dyn Element> {
        let side = matrix.width() + QUIET_ZONE_MODULES * 2;
        let module = QR_SIZE / side as f32;
        let mut column = Flex::column().with_main_axis_size(MainAxisSize::Max);
        for y in 0..side {
            let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);
            for x in 0..side {
                let dark = x >= QUIET_ZONE_MODULES
                    && y >= QUIET_ZONE_MODULES
                    && x - QUIET_ZONE_MODULES < matrix.width()
                    && y - QUIET_ZONE_MODULES < matrix.width()
                    && matrix.is_dark(x - QUIET_ZONE_MODULES, y - QUIET_ZONE_MODULES);
                row.add_child(
                    ConstrainedBox::new(
                        Container::new(Empty::new().finish())
                            .with_background(if dark {
                                ColorU::black()
                            } else {
                                ColorU::white()
                            })
                            .finish(),
                    )
                    .with_width(module)
                    .with_height(module)
                    .finish(),
                );
            }
            column.add_child(row.finish());
        }
        Container::new(
            ConstrainedBox::new(column.finish())
                .with_width(QR_SIZE)
                .with_height(QR_SIZE)
                .finish(),
        )
        .with_background(ColorU::white())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .finish()
    }

    /// The title, the sentences, the link and the copy button, as one column.
    fn render_text(
        &self,
        appearance: &Appearance,
        theme: &warp_core::ui::theme::WarpTheme,
    ) -> Box<dyn Element> {
        let dim = theme.nonactive_ui_text_color().into_solid();
        let line = |text: String| {
            Text::new(text, appearance.ui_font_family(), 14.)
                .with_color(dim)
                .finish()
        };
        let title = Text::new(
            "Remote control".to_owned(),
            appearance.ui_font_family(),
            20.,
        )
        .with_style(Properties::default().weight(Weight::Bold))
        .with_color(theme.main_text_color(theme.background()).into_solid())
        .finish();

        let mut text = Flex::column()
            .with_spacing(8.)
            .with_cross_axis_alignment(CrossAxisAlignment::Start);
        text.add_child(title);
        text.add_child(line(
            "Scan with a phone on this network to drive this conversation from it: watch its \
             record, answer its permission requests, stop it, prompt it. That conversation and \
             no other."
                .to_owned(),
        ));
        text.add_child(
            Text::new(self.url.clone(), appearance.monospace_font_family(), 13.)
                .with_color(theme.main_text_color(theme.background()).into_solid())
                .finish(),
        );
        // The clock is the code's, not the phone's: a control pairing has none
        // (2026-09-06), and the sentence says what ends it instead.
        text.add_child(line(format!(
            "The code in the link is a secret: spendable once, dead at {}. A phone that scans \
             it stays paired until Stop sharing in the footer, this conversation being \
             deleted, or Warp closing.",
            self.expires_at
        )));
        if let Some(ca_url) = &self.ca_url {
            // The link is https:// and the phone trusts nothing yet, so the
            // first scan on a phone is a warning page unless this is done
            // first. Said here, where the person is, rather than on the page
            // the warning stands in front of.
            text.add_child(line(format!(
                "First time on this phone: open {ca_url} in its browser and install the \
                 certificate, then scan."
            )));
        }
        text.add_child(line(format!("The phone may: {}", self.actions.join(", "))));
        text.add_child(ChildView::new(&self.copy_button).finish());
        text.finish()
    }
}

impl Entity for RemoteControlBlock {
    type Event = RemoteControlBlockEvent;
}

impl View for RemoteControlBlock {
    fn ui_name() -> &'static str {
        "RemoteControlBlock"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        if self.should_hide {
            return Empty::new().finish();
        }
        let appearance = Appearance::handle(app).as_ref(app);
        let theme = appearance.theme();

        // Built twice because an element is consumed by the tree it joins, and
        // the switch below needs one for each layout.
        let qr = |view: &Self| -> Box<dyn Element> {
            match &view.matrix {
                Some(matrix) => view.render_qr(matrix),
                None => Text::new(
                    "The QR could not be drawn; use the link.".to_owned(),
                    appearance.ui_font_family(),
                    14.,
                )
                .with_color(theme.nonactive_ui_text_color().into_solid())
                .finish(),
            }
        };
        let beside = Flex::row()
            .with_spacing(24.)
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(qr(self))
            // `Expanded`, not `Align`: the text takes what the QR leaves and
            // wraps inside it, which is what the plugin-instructions block
            // does for the same shape.
            .with_child(Expanded::new(1., self.render_text(appearance, theme)).finish())
            .finish();
        let below = Flex::column()
            .with_spacing(16.)
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(qr(self))
            .with_child(self.render_text(appearance, theme))
            .finish();
        let content = SizeConstraintSwitch::new(
            beside,
            [(
                SizeConstraintCondition::WidthLessThan(STACK_BELOW_WIDTH),
                below,
            )],
        )
        .finish();

        let body = Container::new(content)
            .with_horizontal_padding(*super::PADDING_LEFT)
            .with_vertical_padding(16.)
            .with_border(
                Border::new(1.)
                    .with_sides(true, false, true, false)
                    .with_border_fill(theme.outline()),
            )
            .finish();

        Stack::new()
            .with_child(body)
            .with_positioned_child(
                ChildView::new(&self.close_button).finish(),
                OffsetPositioning::offset_from_parent(
                    vec2f(-8., 8.),
                    ParentOffsetBounds::ParentBySize,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            )
            .finish()
    }
}

impl TypedActionView for RemoteControlBlock {
    type Action = RemoteControlBlockAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            RemoteControlBlockAction::Close => {
                self.should_hide = true;
                ctx.emit(RemoteControlBlockEvent::Close);
                ctx.notify();
            }
            RemoteControlBlockAction::CopyLink => {
                ctx.clipboard()
                    .write(ClipboardContent::plain_text(self.url.clone()));
                let window_id = ctx.window_id();
                ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                    toast_stack.add_ephemeral_toast(
                        DismissibleToast::success("Remote control link copied.".to_owned()),
                        window_id,
                        ctx,
                    );
                });
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum RemoteControlBlockAction {
    Close,
    CopyLink,
}

pub(crate) enum RemoteControlBlockEvent {
    Close,
}

impl super::TerminalView {
    /// `/remote-control`, the fork's way: hand the pane's conversation to a
    /// phone. Mints a code scoped to that conversation, puts the QR in the
    /// pane, and copies the link, which is upstream's own gesture for the chip.
    pub(crate) fn start_fork_remote_control(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(conversation_id) = self.active_conversation_id(ctx) else {
            self.show_error_toast(
                "Nothing to hand over: start a conversation in the agent panel first.".to_owned(),
                ctx,
            );
            return;
        };
        let conversation_id = conversation_id.to_string();
        let result = match crate::local_control::remote_control::start(&conversation_id, ctx) {
            Ok(result) => result,
            Err(error) => {
                self.show_error_toast(format!("Remote control: {}", error.message), ctx);
                return;
            }
        };
        self.remove_remote_control_blocks(ctx);
        ctx.clipboard()
            .write(ClipboardContent::plain_text(result.url.clone()));
        let block = ctx.add_typed_action_view(|ctx| RemoteControlBlock::new(&result, ctx));
        ctx.subscribe_to_view(&block, |view, block, event, ctx| match event {
            RemoteControlBlockEvent::Close => {
                view.remove_remote_control_block(block.clone(), ctx);
            }
        });
        self.insert_rich_content(
            Some(crate::terminal::model::rich_content::RichContentType::RemoteControlBlock),
            block,
            Some(super::rich_content::RichContentMetadata::RemoteControlBlock),
            super::rich_content::RichContentInsertionPosition::Append {
                insert_below_long_running_block: false,
            },
            ctx,
        );
        let window_id = ctx.window_id();
        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            toast_stack.add_ephemeral_toast(
                DismissibleToast::default(
                    "Remote control: scan the QR in the pane (link copied).".to_owned(),
                ),
                window_id,
                ctx,
            );
        });
    }

    /// *Stop sharing*: take the conversation back. Every phone paired for it
    /// and every unscanned code for it stop working.
    pub(crate) fn stop_fork_remote_control(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(conversation_id) = self.active_conversation_id(ctx) else {
            return;
        };
        let cut_off = crate::local_control::remote_control::stop(&conversation_id.to_string(), ctx);
        self.remove_remote_control_blocks(ctx);
        let window_id = ctx.window_id();
        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            toast_stack.add_ephemeral_toast(
                DismissibleToast::default(match cut_off {
                    0 => "Remote control stopped; no phone was connected.".to_owned(),
                    1 => "Remote control stopped; the phone was cut off.".to_owned(),
                    n => format!("Remote control stopped; {n} devices were cut off."),
                }),
                window_id,
                ctx,
            );
        });
    }

    pub(crate) fn remove_remote_control_block(
        &mut self,
        block_handle: ViewHandle<RemoteControlBlock>,
        ctx: &mut ViewContext<Self>,
    ) {
        let block_id = block_handle.id();
        self.rich_content_views
            .retain(|rich_content| rich_content.view_id() != block_id);
        self.model
            .lock()
            .block_list_mut()
            .remove_rich_content(block_id);
        ctx.notify();
    }

    fn remove_remote_control_blocks(&mut self, ctx: &mut ViewContext<Self>) {
        let stale: Vec<_> = self
            .rich_content_views
            .iter()
            .filter(|rich_content| {
                matches!(
                    rich_content.metadata(),
                    Some(super::rich_content::RichContentMetadata::RemoteControlBlock)
                )
            })
            .map(|rich_content| rich_content.view_id())
            .collect();
        if stale.is_empty() {
            return;
        }
        self.rich_content_views
            .retain(|rich_content| !stale.contains(&rich_content.view_id()));
        let mut model = self.model.lock();
        for id in stale {
            model.block_list_mut().remove_rich_content(id);
        }
        drop(model);
        ctx.notify();
    }
}
