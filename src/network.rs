use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct NetIface {
    pub name: String,
    pub ip: IpAddr,
}

pub fn local_interfaces() -> Vec<NetIface> {
    let mut result = vec![NetIface {
        name: "loopback".into(),
        ip: IpAddr::from([127, 0, 0, 1]),
    }];

    if let Ok(ifaces) = NetworkInterface::show() {
        for iface in ifaces {
            for addr in &iface.addr {
                let ip = addr.ip();
                if ip.is_loopback() || ip.is_unspecified() {
                    continue;
                }
                if ip.is_ipv4() {
                    result.push(NetIface {
                        name: iface.name.clone(),
                        ip,
                    });
                }
            }
        }
    }

    result
}

pub fn format_urls(interfaces: &[NetIface], port: u16, is_https: bool) -> Vec<String> {
    let scheme = if is_https { "https" } else { "http" };
    interfaces
        .iter()
        .map(|i| format!("{scheme}://{}:{port}", i.ip))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_interfaces_includes_loopback() {
        let ifaces = local_interfaces();
        assert!(ifaces.iter().any(|i| i.ip.is_loopback()));
    }

    #[test]
    fn test_format_urls() {
        let ifaces = vec![NetIface {
            name: "lo".into(),
            ip: IpAddr::from([127, 0, 0, 1]),
        }];
        let urls = format_urls(&ifaces, 4701, false);
        assert_eq!(urls, vec!["http://127.0.0.1:4701"]);
    }

    #[test]
    fn test_format_urls_https() {
        let ifaces = vec![NetIface {
            name: "lo".into(),
            ip: IpAddr::from([127, 0, 0, 1]),
        }];
        let urls = format_urls(&ifaces, 4801, true);
        assert_eq!(urls, vec!["https://127.0.0.1:4801"]);
    }
}
