//! The questions the app can ask, one of each at a time (= `stores/dialogs.svelte.ts`):
//! "are you sure", and the password a server wants; plus whether the command palette and
//! the shortcuts sheet are open. Async flows ask through [`super::Bus::confirm`] and
//! [`super::Bus::password`]; the shell's dialog host draws what is pending here and calls
//! [`Dialogs::answer`] / [`Dialogs::answer_password`].

use tokio::sync::oneshot;

/// "Are you sure", in the words of what it is about.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfirmRequest {
    pub title: String,
    pub message: String,
    /// Words on the button that acts: "Delete 3 mods", not "OK".
    pub confirm_label: Option<String>,
    pub cancel_label: Option<String>,
    pub danger: bool,
}

impl ConfirmRequest {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            ..Self::default()
        }
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn confirm_label(mut self, label: impl Into<String>) -> Self {
        self.confirm_label = Some(label.into());
        self
    }
}

/// The player's answer to a password request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordAnswer {
    pub password: String,
    /// Keep it with the favourite.
    pub save: bool,
}

#[derive(Default)]
pub struct Dialogs {
    pending: Option<(ConfirmRequest, oneshot::Sender<bool>)>,
    password: Option<(String, oneshot::Sender<Option<PasswordAnswer>>)>,
    /// The command palette (Ctrl+K).
    pub palette: bool,
    /// The sheet of keyboard shortcuts (?).
    pub shortcuts: bool,
}

impl Dialogs {
    /// A new question; a question already showing is answered "no".
    pub fn confirm(&mut self, req: ConfirmRequest, reply: oneshot::Sender<bool>) {
        if let Some((_, old)) = self.pending.replace((req, reply)) {
            let _ = old.send(false);
        }
    }

    /// The question showing, for the dialog host.
    pub fn pending(&self) -> Option<&ConfirmRequest> {
        self.pending.as_ref().map(|(r, _)| r)
    }

    pub fn answer(&mut self, yes: bool) {
        if let Some((_, tx)) = self.pending.take() {
            let _ = tx.send(yes);
        }
    }

    /// Ask the password of `server`; a request already showing is given up.
    pub fn ask_password(&mut self, server: String, reply: oneshot::Sender<Option<PasswordAnswer>>) {
        if let Some((_, old)) = self.password.replace((server, reply)) {
            let _ = old.send(None);
        }
    }

    /// The server whose password is asked, for the dialog host.
    pub fn password_for(&self) -> Option<&str> {
        self.password.as_ref().map(|(s, _)| s.as_str())
    }

    pub fn answer_password(&mut self, a: Option<PasswordAnswer>) {
        if let Some((_, tx)) = self.password.take() {
            let _ = tx.send(a);
        }
    }

    /// Something is being asked: keyboard shortcuts behind it stay quiet.
    pub fn open(&self) -> bool {
        self.pending.is_some() || self.password.is_some() || self.palette || self.shortcuts
    }
}
