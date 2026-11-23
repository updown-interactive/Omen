//! Project-scoped agent chat state.
use super::*;
use services::agent::Outcome;

#[derive(Clone)]
pub struct Message {
    pub user: bool,
    pub text: String,
}

#[derive(Clone, Default)]
pub struct Chat {
    pub messages: Vec<Message>,
    pub busy: bool,
    pub stopping: bool,
    pub status: String,
    thread: Option<String>,
}

impl StudioEnvironment {
    pub(crate) fn send_agent_prompt(&self) {
        if self.agent_chat.read(|chat| chat.busy) {
            return;
        }
        let Some(root) = self.workspace.read(|state| state.root.clone()) else {
            return;
        };
        let prompt = self.agent_prompt.read(|text| text.trim().to_owned());
        if prompt.is_empty() {
            return;
        }
        if prompt.len() > 32_768 {
            self.agent_chat
                .update(|chat| chat.status = "Keep the prompt under 32 KB.".into());
            return;
        }
        let id = self.agent_request.get().wrapping_add(1);
        let thread = self.agent_chat.read(|chat| chat.thread.clone());
        match self.agent_client.send(id, root, thread, prompt.clone()) {
            Ok(task) => {
                self.agent_request.set(id);
                *self.agent_task.borrow_mut() = Some(task);
                self.agent_prompt.set(String::new());
                self.agent_chat.update(|chat| {
                    chat.messages.push(Message {
                        user: true,
                        text: prompt,
                    });
                    chat.status.clear();
                    chat.busy = true;
                });
            }
            Err(error) => self
                .agent_chat
                .update(|chat| chat.status = format!("Could not start the agent: {error}")),
        }
    }

    pub(crate) fn stop_agent(&self) {
        if let Some(task) = self.agent_task.borrow().as_ref() {
            task.stop();
            self.agent_chat.update(|chat| chat.stopping = true);
        }
    }

    pub(crate) fn new_agent_chat(&self) {
        self.agent_request
            .set(self.agent_request.get().wrapping_add(1));
        self.agent_task.borrow_mut().take();
        self.agent_prompt.set(String::new());
        self.agent_chat.set(Chat::default());
    }

    pub(super) async fn receive_agent_updates(&self) {
        let mut events = self.agent_client.events.lock().await;
        while let Some(event) = events.next().await {
            if self.agent_request.get() != event.id {
                continue;
            }
            self.agent_task.borrow_mut().take();
            self.agent_chat.update(|chat| {
                chat.busy = false;
                chat.stopping = false;
                match event.outcome {
                    Outcome::Completed(reply) => {
                        if reply.thread.is_some() {
                            chat.thread = reply.thread;
                        }
                        chat.messages.push(Message {
                            user: false,
                            text: reply.text,
                        });
                    }
                    Outcome::Failed(message) => chat.status = message,
                    Outcome::Stopped => chat.status = "Stopped.".into(),
                }
            });
        }
    }
}
