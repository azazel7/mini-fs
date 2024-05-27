use notify_rust::Notification;
use std::fmt;

pub struct Logger {
    appname: String,
    notification: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum EventType {
    Open,
    Close,
    OpenDir,
    CloseDir,
}

impl Logger {
    pub const fn new(appname: String, notification: bool) -> Self {
        Self {
            appname,
            notification,
        }
    }
    pub fn log(&self, event_type: EventType, message: &str) {
        if self.notification {
            let _ = Notification::new()
                .summary(&self.appname)
                .appname(&self.appname)
                .body(&format!("{event_type} {message}"))
                .timeout(0)
                .show();
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Self::Open => "Open",
            Self::Close => "Close",
            Self::OpenDir => "OpenDir",
            Self::CloseDir => "CloseDir",
        };
        write!(f, "{s}")
    }
}
