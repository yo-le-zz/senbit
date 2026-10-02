use super::level::LogLevel;

pub struct LogRecord<'a> {
    pub timestamp: String,
    pub level: LogLevel,
    pub component: &'a str,
    pub message: String,
}

impl<'a> LogRecord<'a> {
    pub fn format(&self) -> String {
        format!(
            "{} [{}] {}: {}\n",
            self.timestamp,
            self.level.as_str(),
            self.component,
            self.message,
        )
    }
}