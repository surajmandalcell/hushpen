//! Remote candidates from supplied configuration and status text, without IO.

use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteHost {
    pub name: String,
    pub address: String,
    pub ssh_alias: Option<String>,
    pub tailscale_address: Option<String>,
    identities: Vec<String>,
}

impl RemoteHost {
    pub fn secondary_line(&self) -> String {
        match (&self.ssh_alias, &self.tailscale_address) {
            (Some(_), Some(address)) => {
                format!("From ~/.ssh/config · From Tailscale · {address}")
            }
            (Some(_), None) => "From ~/.ssh/config".to_owned(),
            (None, Some(address)) => format!("From Tailscale · {address}"),
            (None, None) => self.address.clone(),
        }
    }

    /// A typed host is a separate entry; it does not imply discovery or trust.
    pub fn typed(input: &str) -> Option<Self> {
        let host = input.trim();
        valid_address(host).then(|| Self {
            name: host.to_owned(),
            address: host.to_owned(),
            ssh_alias: None,
            tailscale_address: None,
            identities: vec![identity(host)],
        })
    }
}

/// SSH aliases stay as connection targets, even when Tailscale provides their identity.
pub fn remote_hosts(
    ssh_config: &str,
    tailscale_status: &str,
) -> Result<Vec<RemoteHost>, serde_json::Error> {
    let mut hosts = Vec::new();
    for host in ssh_hosts(ssh_config)
        .into_iter()
        .chain(tailscale_hosts(tailscale_status)?)
    {
        merge(&mut hosts, host);
    }
    Ok(hosts)
}

/// Includes must be read by the caller; this parser never opens configuration files.
pub fn ssh_hosts(config: &str) -> Vec<RemoteHost> {
    let mut hosts = Vec::new();
    let mut block = Vec::new();
    let mut hostname = None;
    for line in config.lines().chain(std::iter::once("Host")) {
        let fields = ssh_fields(line);
        let Some(keyword) = fields.first() else {
            continue;
        };
        if keyword.eq_ignore_ascii_case("host") || keyword.eq_ignore_ascii_case("match") {
            for alias in block.drain(..) {
                let address = hostname.clone().unwrap_or_else(|| String::clone(&alias));
                merge(
                    &mut hosts,
                    RemoteHost {
                        name: alias.clone(),
                        address: address.clone(),
                        ssh_alias: Some(alias.clone()),
                        tailscale_address: None,
                        identities: vec![identity(&alias), identity(&address)],
                    },
                );
            }
            hostname = None;
            if keyword.eq_ignore_ascii_case("host") {
                block.extend(fields.into_iter().skip(1).filter(|host| valid_alias(host)));
            }
        } else if keyword.eq_ignore_ascii_case("hostname") && hostname.is_none() {
            hostname = fields.get(1).filter(|host| valid_address(host)).cloned();
        }
    }
    hosts
}

pub fn tailscale_hosts(status: &str) -> Result<Vec<RemoteHost>, serde_json::Error> {
    if status.trim().is_empty() {
        return Ok(Vec::new());
    }
    let status: serde_json::Value = serde_json::from_str(status)?;
    let mut hosts = Vec::new();
    for peer in status["Peer"]
        .as_object()
        .into_iter()
        .flat_map(|peers| peers.values())
    {
        let addresses: Vec<_> = peer["TailscaleIPs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str()?.parse::<IpAddr>().ok())
            .collect();
        let Some(address) = addresses
            .iter()
            .find(|address| address.is_ipv4())
            .or_else(|| addresses.first())
            .map(ToString::to_string)
        else {
            continue;
        };
        let dns = peer["DNSName"].as_str().filter(|name| valid_address(name));
        let hostname = peer["HostName"].as_str().filter(|name| valid_alias(name));
        let name = hostname.or(dns).unwrap_or(&address).trim_end_matches('.');
        let identities = addresses
            .iter()
            .map(ToString::to_string)
            .chain(dns.into_iter().map(identity))
            .chain(hostname.into_iter().map(identity))
            .collect();
        merge(
            &mut hosts,
            RemoteHost {
                name: name.to_owned(),
                address: address.clone(),
                ssh_alias: None,
                tailscale_address: Some(address),
                identities,
            },
        );
    }
    Ok(hosts)
}

fn merge(hosts: &mut Vec<RemoteHost>, mut host: RemoteHost) {
    // A peer can join two SSH aliases that previously had no common identity.
    let mut index = 0;
    while index < hosts.len() {
        if hosts[index]
            .identities
            .iter()
            .any(|name| host.identities.contains(name))
        {
            let saved = hosts.remove(index);
            host.identities.extend(saved.identities);
            if saved.ssh_alias.is_some() {
                host.name = saved.name;
                host.address = saved.address;
                host.ssh_alias = saved.ssh_alias;
            }
            host.tailscale_address = host.tailscale_address.or(saved.tailscale_address);
            index = 0;
        } else {
            index += 1;
        }
    }
    hosts.push(host);
}

fn identity(host: &str) -> String {
    host.trim_matches(['[', ']'])
        .parse::<IpAddr>()
        .map(|address| address.to_string())
        .unwrap_or_else(|_| host.trim_end_matches('.').to_ascii_lowercase())
}

fn valid_alias(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn valid_address(host: &str) -> bool {
    valid_alias(host)
        || host
            .strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(host)
            .parse::<IpAddr>()
            .is_ok()
}

fn ssh_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quote = None;
    for character in line.chars() {
        match character {
            '#' if quote.is_none() => break,
            '\'' | '"' if quote == Some(character) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(character),
            '=' if quote.is_none() && fields.len() <= 1 && field.is_empty() => {}
            '=' if quote.is_none() && fields.is_empty() => fields.push(std::mem::take(&mut field)),
            character if character.is_whitespace() && quote.is_none() => {
                if !field.is_empty() {
                    fields.push(std::mem::take(&mut field));
                }
            }
            character => field.push(character),
        }
    }
    if !field.is_empty() && quote.is_none() {
        fields.push(field);
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    const SSH: &str = r#"
Host office office-backup
    HostName WORK.tail.example.
Host=storage
    HostName 100.80.0.2
Host *.example !private ?box -oBad
    HostName ignored.example
Host 'quoted-host' # an alias
    HostName "quoted.example"
Host office
    HostName later.example
Match user nobody
    HostName must-not-apply.example
Host standalone
"#;

    const TAILSCALE: &str = r#"{
        "Self": {"HostName":"this-computer", "TailscaleIPs":["100.80.0.99"]},
        "Peer": {
            "one": {"DNSName":"work.tail.example.", "HostName":"work", "TailscaleIPs":["100.80.0.1","fd7a:115c:a1e0::1"],"Online":true},
            "two": {"DNSName":"storage.tail.example.", "HostName":"storage", "TailscaleIPs":["100.80.0.2"],"Online":false},
            "three": {"DNSName":"other.tail.example.", "HostName":"other", "TailscaleIPs":["fd7a:115c:a1e0::3"]},
            "missing": {"HostName":"no-address", "TailscaleIPs":[]}
        }
    }"#;

    #[test]
    fn ssh_config_keeps_concrete_aliases_and_first_hostname() {
        let hosts = ssh_hosts(SSH);
        assert_eq!(hosts.len(), 4);
        let office = hosts.iter().find(|host| host.name == "office").unwrap();
        assert_eq!(office.address, "WORK.tail.example.");
        assert_eq!(office.secondary_line(), "From ~/.ssh/config");
        assert!(
            hosts
                .iter()
                .any(|host| host.name == "quoted-host" && host.address == "quoted.example")
        );
        assert!(
            hosts
                .iter()
                .any(|host| host.name == "standalone" && host.address == "standalone")
        );
    }

    #[test]
    fn tailscale_includes_offline_peers_and_excludes_self_and_missing_addresses() {
        let hosts = tailscale_hosts(TAILSCALE).unwrap();
        assert_eq!(hosts.len(), 3);
        let storage = hosts.iter().find(|host| host.name == "storage").unwrap();
        assert_eq!(storage.secondary_line(), "From Tailscale · 100.80.0.2");
        assert!(hosts.iter().any(|host| host.address == "fd7a:115c:a1e0::3"));
        assert!(tailscale_hosts("").unwrap().is_empty());
        assert!(tailscale_hosts("not JSON").is_err());
    }

    #[test]
    fn merged_list_matches_dns_and_addresses_but_keeps_ssh_targets() {
        let hosts = remote_hosts(SSH, TAILSCALE).unwrap();
        assert_eq!(hosts.len(), 5);
        let office = hosts.iter().find(|host| host.name == "office").unwrap();
        assert_eq!(office.ssh_alias.as_deref(), Some("office"));
        assert_eq!(office.tailscale_address.as_deref(), Some("100.80.0.1"));
        assert_eq!(
            office.secondary_line(),
            "From ~/.ssh/config · From Tailscale · 100.80.0.1"
        );
        assert_eq!(
            hosts.iter().filter(|host| host.name == "storage").count(),
            1
        );
    }

    #[test]
    fn one_peer_joins_aliases_for_its_ipv4_and_ipv6_addresses() {
        let hosts = remote_hosts(
            "Host first\nHostName 100.80.0.1\nHost second\nHostName fd7a:115c:a1e0::1",
            TAILSCALE,
        )
        .unwrap();
        assert_eq!(hosts.len(), 3);
        assert_eq!(
            hosts
                .iter()
                .filter(|host| host.tailscale_address.as_deref() == Some("100.80.0.1"))
                .count(),
            1
        );
    }

    #[test]
    fn typed_hosts_accept_names_and_ip_addresses_without_command_arguments() {
        for host in [
            "server",
            "office.example",
            "192.0.2.1",
            "2001:db8::1",
            "[2001:db8::1]",
        ] {
            assert_eq!(
                RemoteHost::typed(&format!(" {host} ")).unwrap().address,
                host
            );
        }
        for host in [
            "",
            "   ",
            "-oProxyCommand=bad",
            "host name",
            "user@host",
            "host;command",
            "host\nother",
            "[not-ip]",
        ] {
            assert!(RemoteHost::typed(host).is_none(), "{host}");
        }
    }
}
