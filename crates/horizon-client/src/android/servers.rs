//! The Android launcher's Server Manager.
//!
//! The servers themselves live in the same `servers.dat` the game's Multiplayer screen uses, so
//! adding, editing, reordering or removing one here is visible there too. Favorites and groups are
//! launcher-only, so they live beside it in `android-servers.json`; records are matched by name and
//! address so a game-side rename keeps them when the address still matches.
//!
//! Connections are not reimplemented. A launch writes the chosen address to the launcher's existing
//! server setting and asks the launcher's existing play path to start the game.
use crate::server_list::{List, Server as SavedServer};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, net::Ipv4Addr, net::Ipv6Addr, path::Path, path::PathBuf};

/// Launcher-only favorites and groups; the game does not read this file.
pub const META_FILE: &str = "android-servers.json";
pub const MAX_NAME: usize = 64;
pub const MAX_GROUP: usize = 32;
/// A server icon is a 64x64 PNG in vanilla. Ignore anything far larger before sending it to
/// Android's bitmap decoder.
pub const MAX_ICON_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
struct Record {
    #[serde(default)]
    name: String,
    #[serde(default)]
    ip: String,
    #[serde(default)]
    favorite: bool,
    #[serde(default)]
    group: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    records: Vec<Record>,
}

/// A validated server address. The raw spelling is kept in `servers.dat`; this is only for
/// showing the host and port separately and for rejecting malformed input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    pub host: String,
    pub port: u16,
}

impl Address {
    pub fn display(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// ServerAddress.parseString, with checks the game only discovers after trying to connect.
pub fn validate_address(text: &str) -> Result<Address, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Enter a server address.".into());
    }
    if text.len() > 255 {
        return Err("That server address is too long.".into());
    }
    if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("A server address cannot contain spaces.".into());
    }
    if text.contains("://")
        || text.contains('/')
        || text.contains('?')
        || text.contains('#')
        || text.contains('@')
    {
        return Err("Enter a hostname or IP address, not a web address.".into());
    }

    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        let (host, tail) = rest
            .split_once(']')
            .ok_or("Close the IPv6 address with ].")?;
        if host.is_empty() {
            return Err("Enter an IPv6 address between the brackets.".into());
        }
        if host.parse::<Ipv6Addr>().is_err() {
            return Err("That is not a valid IPv6 address.".into());
        }
        let port = if tail.is_empty() {
            25565
        } else {
            let port = tail
                .strip_prefix(':')
                .ok_or("Use [address]:port for an IPv6 server.")?;
            parse_port(port)?
        };
        (host.to_owned(), port)
    } else if text.matches(':').count() > 1 {
        // An unbracketed IPv6 address. Vanilla treats a trailing group as part of the address,
        // never as a port, so an explicit port must use brackets.
        if text.parse::<Ipv6Addr>().is_err() {
            return Err("Use [address]:port for an IPv6 server with a port.".into());
        }
        (text.to_owned(), 25565)
    } else if let Some((host, port)) = text.rsplit_once(':') {
        (host.to_owned(), parse_port(port)?)
    } else {
        (text.to_owned(), 25565)
    };

    validate_host(&host)?;
    Ok(Address { host, port })
}

fn parse_port(text: &str) -> Result<u16, String> {
    let port = text
        .parse::<u16>()
        .map_err(|_| "The port must be a number from 1 to 65535.".to_owned())?;
    if port == 0 {
        return Err("The port must be a number from 1 to 65535.".into());
    }
    Ok(port)
}

fn validate_host(host: &str) -> Result<(), String> {
    if host.is_empty() {
        return Err("Enter a hostname or IP address before the port.".into());
    }
    if !host.is_ascii() {
        return Err("Use an ASCII hostname or an IP address.".into());
    }
    if host.parse::<Ipv4Addr>().is_ok() || host.parse::<Ipv6Addr>().is_ok() {
        return Ok(());
    }
    let labels: Vec<&str> = host.trim_end_matches('.').split('.').collect();
    if labels
        .iter()
        .all(|label| !label.is_empty() && label.chars().all(|c| c.is_ascii_digit()))
    {
        return Err("That is not a valid IPv4 address.".into());
    }
    if labels.len() > 1 && labels.iter().any(|label| label.is_empty()) {
        return Err("That hostname has an empty part.".into());
    }
    if labels.iter().any(|label| label.is_empty()) {
        return Err("Enter a hostname or IP address.".into());
    }
    if host.len() > 253 || labels.iter().any(|label| label.len() > 63) {
        return Err("That hostname is too long.".into());
    }
    for label in labels {
        if !label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err("Use letters, numbers, dots and hyphens in the hostname.".into());
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("A hostname part cannot start or end with a hyphen.".into());
        }
    }
    Ok(())
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.chars().count() > MAX_NAME {
        return Err(format!("Server names can be up to {MAX_NAME} characters."));
    }
    Ok(if name.is_empty() {
        "Minecraft Server".to_owned()
    } else {
        name.to_owned()
    })
}

fn clean_group(group: &str) -> Result<String, String> {
    let group = group.trim();
    if group.chars().count() > MAX_GROUP {
        return Err(format!("Group names can be up to {MAX_GROUP} characters."));
    }
    if group.chars().any(char::is_control) {
        return Err("A group name cannot contain line breaks.".into());
    }
    Ok(group.to_owned())
}

fn same_address(a: &str, b: &str) -> bool {
    let norm = |ip: &str| {
        let ip = ip.trim().to_ascii_lowercase();
        ip.strip_suffix(":25565").map(str::to_owned).unwrap_or(ip)
    };
    norm(a) == norm(b)
}

fn reconcile(servers: &[SavedServer], old: Vec<Record>) -> Vec<Record> {
    let mut used = vec![false; old.len()];
    let take = |used: &mut Vec<bool>, f: &dyn Fn(&Record) -> bool| {
        old.iter()
            .enumerate()
            .find(|(i, record)| !used[*i] && f(record))
            .map(|(i, _)| i)
            .inspect(|i| used[*i] = true)
    };
    servers
        .iter()
        .map(|server| {
            let by_both = take(&mut used, &|record| {
                record.name == server.name && same_address(&record.ip, &server.ip)
            });
            let by_ip = by_both.or_else(|| take(&mut used, &|r| same_address(&r.ip, &server.ip)));
            let by_name = by_ip.or_else(|| {
                let matches = old
                    .iter()
                    .enumerate()
                    .filter(|(i, r)| !used[*i] && !r.name.is_empty() && r.name == server.name)
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>();
                (matches.len() == 1)
                    .then(|| matches[0])
                    .inspect(|i| used[*i] = true)
            });
            let mut record = by_name.map_or_else(Record::default, |i| old[i].clone());
            record.name = server.name.clone();
            record.ip = server.ip.clone();
            record
        })
        .collect()
}

/// The Server Manager's view of one instance's list.
pub struct Manager {
    home: PathBuf,
    list: List,
    records: Vec<Record>,
}

impl Manager {
    pub fn load(home: &Path) -> Self {
        let list = List::load(home);
        let records = reconcile(&list.servers(), read_meta(home));
        Self {
            home: home.to_path_buf(),
            list,
            records,
        }
    }

    /// The JSON the Compose sheet renders. Addresses that predate the manager but no longer
    /// parse keep their raw spelling and get no invented port.
    pub fn state(&self, instance: &str) -> Value {
        let groups: Vec<&str> = self
            .records
            .iter()
            .map(|record| record.group.as_str())
            .filter(|group| !group.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let servers: Vec<Value> = self
            .list
            .servers()
            .into_iter()
            .enumerate()
            .map(|(index, server)| {
                let record = self.records.get(index).cloned().unwrap_or_default();
                let address = validate_address(&server.ip).ok();
                let icon = server
                    .icon
                    .filter(|bytes| bytes.len() <= MAX_ICON_BYTES)
                    .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes));
                json!({
                    "index": index,
                    "name": server.name,
                    "address": server.ip,
                    "host": address.as_ref().map(|address| address.host.clone()),
                    "port": address.as_ref().map(|address| address.port),
                    "display_address": address.as_ref().map(|address| address.display()),
                    "icon": icon,
                    "favorite": record.favorite,
                    "group": record.group,
                })
            })
            .collect();
        json!({
            "instance": instance,
            "servers": servers,
            "groups": groups,
            "max_name": MAX_NAME,
            "max_group": MAX_GROUP,
        })
    }

    pub fn add(
        &mut self,
        name: &str,
        address: &str,
        group: &str,
        favorite: bool,
    ) -> Result<(), String> {
        let address = address.trim().to_owned();
        validate_address(&address)?;
        let name = clean_name(name)?;
        let group = clean_group(group)?;
        self.list.add(&name, &address)?;
        let server = self
            .list
            .servers()
            .last()
            .cloned()
            .ok_or("The server was not added.")?;
        self.records.push(Record {
            name: server.name,
            ip: server.ip,
            favorite,
            group,
        });
        self.save()
    }

    pub fn update(
        &mut self,
        index: usize,
        name: &str,
        address: &str,
        group: &str,
        favorite: bool,
    ) -> Result<(), String> {
        let address = address.trim().to_owned();
        validate_address(&address)?;
        let name = clean_name(name)?;
        let group = clean_group(group)?;
        if self.records.len() != self.list.servers().len() {
            return Err("The server list changed. Reopen it and try again.".into());
        }
        let server = self.list.edit(index, &name, &address)?;
        let record = self
            .records
            .get_mut(index)
            .ok_or_else(|| format!("There is no server {}.", index + 1))?;
        *record = Record {
            name: server.name,
            ip: server.ip,
            favorite,
            group,
        };
        self.save()
    }

    pub fn remove(&mut self, index: usize) -> Result<SavedServer, String> {
        let server = self.list.remove(index)?;
        if index < self.records.len() {
            self.records.remove(index);
        }
        self.save()?;
        Ok(server)
    }

    pub fn duplicate(&mut self, index: usize) -> Result<SavedServer, String> {
        let copy = self.list.duplicate(index)?;
        let source = self.records.get(index).cloned().unwrap_or_default();
        self.records.insert(
            (index + 1).min(self.records.len()),
            Record {
                name: copy.name.clone(),
                ip: copy.ip.clone(),
                ..source
            },
        );
        self.save()?;
        Ok(copy)
    }

    pub fn move_by(&mut self, index: usize, delta: i64) -> Result<(), String> {
        let target = index as i64 + delta;
        if target < 0 || target as usize >= self.list.servers().len() {
            return Err("That server is already at the edge of the list.".into());
        }
        if target as usize == index {
            return Ok(());
        }
        if self.records.len() != self.list.servers().len() {
            return Err("The server list changed. Reopen it and try again.".into());
        }
        self.list.move_by(index, delta)?;
        let record = self.records.remove(index);
        self.records.insert(target as usize, record);
        self.save()
    }

    /// Flips a launcher-only favorite without rewriting `servers.dat`.
    pub fn toggle_favorite(&mut self, index: usize) -> Result<bool, String> {
        let record = self
            .records
            .get_mut(index)
            .ok_or_else(|| format!("There is no server {}.", index + 1))?;
        record.favorite = !record.favorite;
        let favorite = record.favorite;
        self.save_meta()?;
        Ok(favorite)
    }

    pub fn address(&self, index: usize) -> Result<String, String> {
        self.list
            .servers()
            .get(index)
            .map(|server| server.ip.clone())
            .ok_or_else(|| format!("There is no server {}.", index + 1))
    }

    pub fn addresses(&self) -> Vec<String> {
        self.list
            .servers()
            .into_iter()
            .map(|server| server.ip)
            .collect()
    }

    fn save(&self) -> Result<(), String> {
        self.list.save(&self.home)?;
        self.save_meta()
    }

    fn save_meta(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&Store {
            version: 1,
            records: self.records.clone(),
        })
        .map_err(|error| error.to_string())?;
        let path = self.home.join(META_FILE);
        let temporary = self.home.join("android-servers.json.tmp");
        fs::create_dir_all(&self.home)
            .and_then(|()| fs::write(&temporary, bytes))
            .and_then(|()| fs::rename(&temporary, &path))
            .map_err(|error| format!("Could not save server favorites and groups: {error}"))
    }
}

fn read_meta(home: &Path) -> Vec<Record> {
    let path = home.join(META_FILE);
    match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<Store>(&bytes) {
            Ok(store) => store.records,
            Err(_) => {
                // Keep the unreadable file for a person to inspect; start favorites and groups
                // fresh instead of blocking the server list.
                let _ = fs::rename(&path, home.join("android-servers.json.corrupt"));
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    }
}

/// A manual status check's lifecycle. Status is only shown after the player asks for it, so no
/// background polling is needed.
#[derive(Clone, Debug, PartialEq)]
pub enum Ping {
    Pending,
    Online {
        players: Option<(i64, i64)>,
        latency_ms: u128,
    },
    Offline(String),
}

impl Ping {
    pub fn from_result(result: Result<(Value, u128), String>) -> Self {
        match result {
            Ok((status, latency_ms)) => {
                let players = status.get("players").and_then(|players| {
                    Some((
                        players.get("online")?.as_i64()?,
                        players.get("max")?.as_i64()?,
                    ))
                });
                Self::Online {
                    players,
                    latency_ms,
                }
            }
            Err(error) => {
                let message = error.replace(['\r', '\n'], " ");
                let message: String = message.chars().take(160).collect();
                Self::Offline(if message.trim().is_empty() {
                    "No status reply.".into()
                } else {
                    format!("No status reply: {message}")
                })
            }
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::Pending => json!({"state": "pending"}),
            Self::Online {
                players,
                latency_ms,
            } => json!({
                "state": "online",
                "players": players.map(|(online, max)| json!({"online": online, "max": max})),
                "latency_ms": (*latency_ms).min(u64::MAX as u128) as u64,
            }),
            Self::Offline(error) => json!({"state": "offline", "error": error}),
        }
    }
}
