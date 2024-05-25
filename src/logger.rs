use notify_rust::Notification;
use std::fmt;

pub struct Logger {
    appname: String,
    notification: bool,
}

#[derive(Debug)]
pub enum EventType {
    Open,
    Close,
    OpenDir,
    CloseDir,
}

impl Logger {
    pub fn new(appname: String, notification: bool) -> Self {
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
            EventType::Open => "Open",
            EventType::Close => "Close",
            EventType::OpenDir => "OpenDir",
            EventType::CloseDir => "CloseDir",
        };
        write!(f, "{}", s)
    }
}
