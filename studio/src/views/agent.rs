//! Agent chat in the project sidebar.
use crate::app::{StudioEnvironment, agent::Message};
use omen::view;

view! {
    pub(super) AgentChat {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let chat = app.agent_chat;
        Layout {
            Layout {
                Text("Agent").fontSize(13).weight(.semibold).fill()
                Button("New chat") { app.new_agent_chat(); }
                    .fontSize(11).padding(5).cornerRadius(5)
                    .hoverBackground(colors.surface_container_high)
            }.mode(.horizontal).frame(.infinity, 28)
            Divider()
            Layout {
                if chat.messages.is_empty() {
                    Layout {
                        Icon("agent-chat").frame(24, 24).color(colors.accent_foreground)
                        Text("Ask Codex about this project.")
                            .fontSize(13).weight(.semibold)
                    }.spacing(9).padding(8)
                }
                for message in chat.messages {
                    ChatMessage(message: message)
                }
                if chat.busy {
                    Text(if chat.stopping { "Stopping…" } else { "Codex is working…" })
                        .fontSize(11).color(colors.on_surface_variant).padding(6)
                }
                if !chat.status.is_empty() {
                    Text(chat.status).fontSize(11).color(colors.on_surface_variant).padding(6)
                }
            }.spacing(8).fill().followScrollEnd()
            AgentComposer(busy: chat.busy)
        }
        .padding(10).spacing(8).frame(.infinity, .infinity)
        .background(colors.surface_container).color(colors.on_surface)
        .shortcut(if cfg!(target_os = "macos") { "cmd-enter" } else { "ctrl-enter" }, move || app.send_agent_prompt())
    }

    AgentComposer(busy: bool) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let prompt = app.agent_prompt;
        let can_send = !prompt.trim().is_empty() && !busy;
        Layout {
            TextEditor("Message Codex").value($app.agent_prompt)
                .frame(.infinity, 72).fontSize(12)
                .background(colors.surface).cornerRadius(6)
            Layout {
                Text("Read-only project access").fontSize(10).color(colors.on_surface_variant).fill()
                if busy {
                    Button("Stop") { app.stop_agent(); }
                        .frame(54, 25).fontSize(11).cornerRadius(5)
                        .hoverBackground(colors.surface_container_high)
                } else {
                    Button("Send") { app.send_agent_prompt(); }
                        .frame(54, 25).fontSize(11).cornerRadius(5)
                        .background(if can_send { colors.primary } else { colors.surface_container_high })
                        .color(if can_send { colors.on_primary } else { colors.on_surface_variant })
                }
            }.mode(.horizontal).spacing(4)
        }.spacing(7)
    }

    ChatMessage(message: Message) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Layout {
            Text(if message.user { "You" } else { "Codex" })
                .fontSize(10).weight(.semibold).color(colors.on_surface_variant)
            Text(message.text).fontSize(12)
        }
        .spacing(5).padding(8).cornerRadius(6)
        .background(if message.user { colors.selection } else { colors.surface })
        .color(if message.user { colors.on_selection } else { colors.on_surface })
    }
}
