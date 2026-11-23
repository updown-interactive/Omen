//! Reactive toolbar state and coordination for asynchronous run sessions.
use super::*;
use crate::services::runner::{Event, RunSession};
use omen_cli::studio::{RunDevices, RunTarget};

#[derive(Clone, Default)]
pub struct RunMenu {
    pub loading: bool,
    pub devices: RunDevices,
}

#[derive(Clone, Default)]
pub struct RunStatus {
    pub active: bool,
    pub stopping: bool,
    pub failed: bool,
    pub message: String,
}

pub(super) struct ActiveRun {
    id: u64,
    root: PathBuf,
    target: RunTarget,
    session: RunSession,
    restart: bool,
}

impl StudioEnvironment {
    pub(crate) fn discover_run_devices(&self) {
        let Some(root) = self.workspace.read(|state| state.root.clone()) else {
            return;
        };
        let id = self.next_device_scan();
        self.run_menu.set(RunMenu {
            loading: true,
            ..Default::default()
        });
        if let Err(error) = self.runner.discover(id, root) {
            self.show_devices(Err(format!("Could not check devices: {error}")));
        }
    }

    fn next_device_scan(&self) -> u64 {
        let id = self.device_scan.get().wrapping_add(1);
        self.device_scan.set(id);
        id
    }

    fn show_devices(&self, result: Result<RunDevices, String>) {
        let devices = result.unwrap_or_else(|issue| RunDevices {
            issues: vec![issue],
            ..Default::default()
        });
        self.run_menu.set(RunMenu {
            loading: false,
            devices,
        });
    }

    pub(crate) fn start_run(&self, target: RunTarget) {
        if self.run_session.borrow().is_some() {
            return;
        }
        let Some(root) = self.workspace.read(|state| state.root.clone()) else {
            return;
        };
        self.console_output.set(Rc::from(""));
        self.console_visible.set(true);
        let Some(workspace) = self.service.clone() else {
            self.run_failed("Workspace worker is unavailable".into());
            return;
        };
        let id = self.run_sequence.get().wrapping_add(1);
        self.run_sequence.set(id);
        match self
            .runner
            .start(id, root.clone(), target.clone(), workspace)
        {
            Ok(session) => {
                self.run_status.set(RunStatus {
                    active: true,
                    message: format!("Run session · {}", target.name),
                    ..Default::default()
                });
                *self.run_session.borrow_mut() = Some(ActiveRun {
                    id,
                    root,
                    target,
                    session,
                    restart: false,
                });
            }
            Err(error) => self.run_failed(format!("Could not start run: {error}")),
        }
    }

    pub(crate) fn toggle_console(&self) {
        self.console_visible.update(|visible| *visible = !*visible);
    }

    pub(crate) fn stop_run(&self) {
        self.end_run(false);
    }

    pub(crate) fn restart_run(&self) {
        self.end_run(true);
    }

    fn end_run(&self, restart: bool) {
        let mut active = self.run_session.borrow_mut();
        let Some(active) = active.as_mut() else {
            return;
        };
        active.restart = restart;
        active.session.stop();
        self.run_status.update(|status| {
            status.stopping = true;
            status.message = format!(
                "{} · {}",
                if restart { "Restarting" } else { "Stopping" },
                active.target.name
            );
        });
    }

    pub(super) fn reset_run_project(&self) {
        self.next_device_scan();
        self.run_menu.set(RunMenu::default());
        self.console_visible.set(false);
        self.console_output.set(Rc::from(""));
        if self.run_session.borrow().is_some() {
            self.stop_run();
        } else {
            self.run_status.set(RunStatus::default());
        }
    }

    fn run_failed(&self, message: String) {
        self.run_status.set(RunStatus {
            failed: true,
            message,
            ..Default::default()
        });
    }

    pub(super) async fn receive_run_updates(&self) {
        let mut events = self.runner.events.lock().await;
        while let Some(event) = events.next().await {
            match event {
                Event::Devices(id, result) if id == self.device_scan.get() => {
                    self.show_devices(result)
                }
                Event::Devices(_, _) => {}
                Event::Output(id, output) => {
                    let current = self.run_session.borrow().as_ref().is_some_and(|run| {
                        run.id == id
                            && self
                                .workspace
                                .read(|state| state.root.as_ref() == Some(&run.root))
                    });
                    if current {
                        self.console_output.set(Rc::from(output));
                    }
                }
                Event::Finished(id, result) => {
                    if !self
                        .run_session
                        .borrow()
                        .as_ref()
                        .is_some_and(|run| run.id == id)
                    {
                        continue;
                    }
                    let Some(active) = self.run_session.borrow_mut().take() else {
                        continue;
                    };
                    self.run_status.set(RunStatus::default());
                    if !self
                        .workspace
                        .read(|state| state.root.as_ref() == Some(&active.root))
                    {
                        continue;
                    }
                    match result {
                        Ok(()) if active.restart => self.start_run(active.target),
                        Ok(()) => {}
                        Err(error) => self.run_failed(error),
                    }
                }
            }
        }
    }
}
