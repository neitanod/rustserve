use std::net::TcpListener;

pub fn find_port(default: u16, explicit: Option<u16>, strict: bool) -> Result<u16, String> {
    match explicit {
        Some(port) => {
            if is_port_available(port) {
                Ok(port)
            } else {
                Err(format!("port {port} is already in use"))
            }
        }
        None if strict => {
            if is_port_available(default) {
                Ok(default)
            } else {
                Err(format!("port {default} is already in use (strict mode)"))
            }
        }
        None => find_free_port(default)
            .ok_or_else(|| format!("no free port found starting from {default}")),
    }
}

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

fn find_free_port(start: u16) -> Option<u16> {
    (start..=start.saturating_add(100)).find(|&p| is_port_available(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn test_find_free_port_from_default() {
        let port = find_port(19876, None, false).unwrap();
        assert!(port >= 19876);
        assert!(is_port_available(port));
    }

    #[test]
    fn test_explicit_port_busy() {
        let _listener = TcpListener::bind(("0.0.0.0", 19877)).unwrap();
        let result = find_port(19877, Some(19877), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_auto_skips_busy_port() {
        let _listener = TcpListener::bind(("0.0.0.0", 19878)).unwrap();
        let port = find_port(19878, None, false).unwrap();
        assert!(port > 19878);
    }

    #[test]
    fn test_strict_default_busy_fails() {
        let _listener = TcpListener::bind(("0.0.0.0", 19879)).unwrap();
        let result = find_port(19879, None, true);
        assert!(
            result.is_err(),
            "strict mode must not look for the next free port"
        );
    }

    #[test]
    fn test_strict_default_free_returns_default() {
        let port = find_port(19880, None, true).unwrap();
        assert_eq!(port, 19880);
    }

    #[test]
    fn test_non_strict_default_busy_finds_next() {
        let _listener = TcpListener::bind(("0.0.0.0", 19881)).unwrap();
        let port = find_port(19881, None, false).unwrap();
        assert!(port > 19881);
    }
}
