use std::io::IsTerminal;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAction {
    HotReload,
    HotRestart,
    Help,
    Clear,
    Quit,
}

pub fn parse_action(ch: char) -> Option<UserAction> {
    match ch {
        'r' => Some(UserAction::HotReload),
        'R' => Some(UserAction::HotRestart),
        'h' | '?' | 'H' => Some(UserAction::Help),
        'c' | 'C' => Some(UserAction::Clear),
        'q' | 'Q' => Some(UserAction::Quit),
        _ => None,
    }
}

pub struct InteractiveSession {
    receiver: Receiver<UserAction>,
}

impl InteractiveSession {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        if std::io::stdin().is_terminal() {
            std::thread::spawn(move || {
                let term = console::Term::stdout();
                loop {
                    match term.read_char() {
                        Ok(ch) => {
                            if let Some(action) = parse_action(ch) {
                                let is_quit = action == UserAction::Quit;
                                let _ = tx.send(action);
                                if is_quit {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }
        Self { receiver: rx }
    }

    pub fn print_banner() {
        println!();
        println!("  🔥 Hot reload & restart enabled:");
        println!("  • Press \"r\" to hot reload changes.");
        println!("  • Press \"R\" to hot restart the application.");
        println!("  • Press \"h\" for help, or \"q\" to quit.");
        println!();
    }

    pub fn print_help() {
        println!();
        println!("  Omen Interactive Commands:");
        println!("  r  Hot reload (recompile and update running activity)");
        println!("  R  Hot restart (recompile, reset state, and restart application)");
        println!("  c  Clear console screen");
        println!("  q  Quit and close application");
        println!();
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<UserAction> {
        self.receiver.recv_timeout(timeout).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hot_reload_and_restart_actions() {
        assert_eq!(parse_action('r'), Some(UserAction::HotReload));
        assert_eq!(parse_action('R'), Some(UserAction::HotRestart));
        assert_eq!(parse_action('q'), Some(UserAction::Quit));
        assert_eq!(parse_action('Q'), Some(UserAction::Quit));
        assert_eq!(parse_action('h'), Some(UserAction::Help));
        assert_eq!(parse_action('?'), Some(UserAction::Help));
        assert_eq!(parse_action('c'), Some(UserAction::Clear));
        assert_eq!(parse_action('x'), None);
        assert_eq!(parse_action('1'), None);
    }
}
