use std::io::Write;

pub fn format_event(level: &str, component: &str, msg: &str) -> String {
    format!("{level} {component}: {msg}")
}

pub fn log_info(component: &str, msg: &str) {
    let line = format_event("INFO", component, msg);
    let _ = writeln!(std::io::stderr(), "{line}");
}

pub fn log_error(component: &str, msg: &str) {
    let line = format_event("ERROR", component, msg);
    let _ = writeln!(std::io::stderr(), "{line}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_info_line() {
        assert_eq!(
            format_event("INFO", "http", "listening on 0.0.0.0:4701"),
            "INFO http: listening on 0.0.0.0:4701"
        );
    }

    #[test]
    fn formats_error_line() {
        assert_eq!(
            format_event("ERROR", "webdav", "bind failed"),
            "ERROR webdav: bind failed"
        );
    }
}
