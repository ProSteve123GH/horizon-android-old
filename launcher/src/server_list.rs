//! The player's server list, `servers.dat` in an instance's game folder: the game's Multiplayer
//! screen reads and writes the same file. Its format is vanilla's (`ServerList.load`/`save`,
//! `ServerData.read`/`write`): uncompressed NBT, `servers` a list of compounds with `name`, `ip`,
//! an optional `icon` (a PNG in base64), `acceptTextures`, `acceptedCodeOfConduct` and `hidden`.
//! The launcher adds, edits, duplicates, reorders and removes entries; every other field of
//! every entry is written back as it was read.
use base64::Engine;
use horizon_world::nbt::{self, Compound, JavaString, Limits, NamedTag, Tag};
use std::fs;
use std::path::Path;

const FILE: &str = "servers.dat";
/// Horizon's official survival server, named as the game lists it. Its address comes from
/// `launcher/links.json` on main (`backend::Links`), so a move to a new host needs no release.
pub const OFFICIAL_NAME: &str = "Horizon Official";
/// The official server's address until links.json is read, and when it can't be.
pub const OFFICIAL_ADDRESS: &str = horizon_telemetry::server_status::OFFICIAL_ADDRESS;
/// Addresses the official server had before: lists that still hold one are moved to the current one.
pub const OFFICIAL_PREVIOUS: &[&str] = horizon_telemetry::server_status::OFFICIAL_PREVIOUS;
/// Beside servers.dat once the official server was offered, so a player who removes it never
/// gets it back: the address offered, so a new one moves the list once. The game writes the same file.
const OFFICIAL_MARKER: &str = "horizon_official_server.txt";

#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub name: String,
    pub ip: String,
    /// The server's picture, as the game last saw it (PNG).
    pub icon: Option<Vec<u8>>,
}

/// The entries as they are in the file, the hidden ones (servers joined with Direct
/// Connection) too.
pub struct List {
    entries: Vec<Compound>,
}

fn key(k: &str) -> JavaString {
    JavaString::from(k)
}

fn text(entry: &Compound, k: &str) -> String {
    match entry.get(&key(k)) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    }
}

fn hidden(entry: &Compound) -> bool {
    matches!(entry.get(&key("hidden")), Some(Tag::Byte(b)) if *b != 0)
}

fn entry(name: &str, ip: &str) -> Compound {
    [
        (key("name"), Tag::String(name.into())),
        (key("ip"), Tag::String(ip.into())),
        (key("hidden"), Tag::Byte(0)),
    ]
    .into_iter()
    .collect()
}

/// A copied server's name, distinct from the original and still at most 64 characters.
fn copy_name(name: &str) -> String {
    let base = match name.trim() {
        "" => "Minecraft Server",
        name => name,
    };
    const SUFFIX: &str = " (copy)";
    let room = 64 - SUFFIX.chars().count();
    let mut copy: String = base.chars().take(room).collect();
    copy.push_str(SUFFIX);
    copy
}

/// An address as the game connects to it: any case, 25565 when no port is given.
fn same_address(a: &str, b: &str) -> bool {
    let norm = |ip: &str| {
        let ip = ip.trim().to_ascii_lowercase();
        ip.strip_suffix(":25565").map(str::to_owned).unwrap_or(ip)
    };
    norm(a) == norm(b)
}

impl List {
    /// The list in `home`; empty when there is none or it can't be read, as the game does.
    pub fn load(home: &Path) -> List {
        Self::read(home).unwrap_or(List {
            entries: Vec::new(),
        })
    }

    /// The list in `home`, empty when there is none; none when the file is there but can't be read.
    fn read(home: &Path) -> Option<List> {
        let bytes = match fs::read(home.join(FILE)) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(_) => return None,
        };
        let entries = match bytes.is_empty() {
            true => Vec::new(),
            false => match nbt::read_named(&bytes, Limits::default())
                .ok()?
                .0
                .tag
                .get("servers")
            {
                Some(Tag::List(list)) => list
                    .iter()
                    .filter_map(|t| match t {
                        Tag::Compound(c) => Some(c.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
        };
        Some(List { entries })
    }

    /// The servers the Multiplayer screen shows, in its order.
    pub fn servers(&self) -> Vec<Server> {
        self.entries
            .iter()
            .filter(|e| !hidden(e))
            .map(|e| Server {
                name: text(e, "name"),
                ip: text(e, "ip"),
                icon: Some(text(e, "icon"))
                    .filter(|s| !s.is_empty())
                    .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok()),
            })
            .collect()
    }

    /// Adds a server at the end of the shown ones, as the game's Add Server does.
    pub fn add(&mut self, name: &str, ip: &str) -> Result<(), String> {
        let (name, ip) = (name.trim(), ip.trim());
        if ip.is_empty() {
            return Err("A server needs an address.".into());
        }
        let name = if name.is_empty() {
            "Minecraft Server"
        } else {
            name
        };
        let at = self
            .entries
            .iter()
            .rposition(|e| !hidden(e))
            .map_or(0, |i| i + 1);
        self.entries.insert(at, entry(name, ip));
        Ok(())
    }

    /// Edits the `index`th shown server, keeping its picture when the address is unchanged and
    /// every field the launcher does not understand. A blank name gets the game's default.
    pub fn edit(&mut self, index: usize, name: &str, ip: &str) -> Result<Server, String> {
        let (name, ip) = (name.trim(), ip.trim());
        if ip.is_empty() {
            return Err("A server needs an address.".into());
        }
        let name = if name.is_empty() {
            "Minecraft Server"
        } else {
            name
        };
        let at = self
            .positions()
            .get(index)
            .copied()
            .ok_or_else(|| format!("There is no server {}.", index + 1))?;
        if !same_address(&text(&self.entries[at], "ip"), ip) {
            self.entries[at].remove(&key("icon"));
        }
        self.entries[at].insert(key("name"), Tag::String(name.into()));
        self.entries[at].insert(key("ip"), Tag::String(ip.into()));
        Ok(self.servers()[index].clone())
    }

    /// Copies the `index`th shown server directly after it, keeping every field and giving the
    /// copy a distinct name. A server list may contain two entries for one address.
    pub fn duplicate(&mut self, index: usize) -> Result<Server, String> {
        let at = self
            .positions()
            .get(index)
            .copied()
            .ok_or_else(|| format!("There is no server {}.", index + 1))?;
        let mut copy = self.entries[at].clone();
        let name = copy_name(&text(&copy, "name"));
        copy.insert(key("name"), Tag::String(name.as_str().into()));
        self.entries.insert(at + 1, copy);
        Ok(self.servers()[index + 1].clone())
    }

    /// Moves the `index`th shown server by `delta` places among the shown servers. Hidden
    /// entries stay where they are. Moving past either end is an error, not a clamp.
    pub fn move_by(&mut self, index: usize, delta: i64) -> Result<(), String> {
        let positions = self.positions();
        if index >= positions.len() {
            return Err(format!("There is no server {}.", index + 1));
        }
        let target = index as i64 + delta;
        if target < 0 || target as usize >= positions.len() {
            return Err("That server is already at the edge of the list.".into());
        }
        let target = target as usize;
        if target == index {
            return Ok(());
        }
        let (from, to) = (positions[index], positions[target]);
        let entry = self.entries.remove(from);
        // Removing an earlier entry shifts a later target one place left.
        self.entries.insert(if from < to { to - 1 } else { to }, entry);
        Ok(())
    }

    /// The actual positions of the shown entries, in their shown order.
    fn positions(&self) -> Vec<usize> {
        (0..self.entries.len())
            .filter(|&i| !hidden(&self.entries[i]))
            .collect()
    }

    /// Removes the `index`th shown server.
    pub fn remove(&mut self, index: usize) -> Result<Server, String> {
        let server = self
            .servers()
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("There is no server {}.", index + 1))?;
        let at = (0..self.entries.len())
            .filter(|&i| !hidden(&self.entries[i]))
            .nth(index)
            .unwrap();
        self.entries.remove(at);
        Ok(server)
    }

    /// Writes the list as the game does: a new file first, the old one kept as `servers.dat_old`.
    pub fn save(&self, home: &Path) -> Result<(), String> {
        let root = NamedTag {
            name: "".into(),
            tag: Tag::compound([(
                "servers",
                Tag::List(self.entries.iter().cloned().map(Tag::Compound).collect()),
            )]),
        };
        let bytes = nbt::write_named(&root, Limits::default()).map_err(|e| e.to_string())?;
        let (new, current) = (home.join("servers.dat_new"), home.join(FILE));
        let written = fs::create_dir_all(home)
            .and_then(|()| fs::write(&new, bytes))
            .and_then(|()| match current.exists() {
                true => fs::copy(&current, home.join("servers.dat_old")).map(|_| ()),
                false => Ok(()),
            })
            .and_then(|()| fs::rename(&new, &current));
        written.map_err(|e| format!("Could not save the server list: {e}"))
    }

    /// Points the entries for a previous official address at `address`, each keeping the player's
    /// name, picture and every other field. The shown list keeps one entry for the server: an old
    /// one goes when the list already has `address`. Whether anything changed.
    fn move_official(&mut self, address: &str, previous: &[String]) -> bool {
        let old = |ip: &str| {
            !same_address(ip, address)
                && previous
                    .iter()
                    .any(|p| !p.trim().is_empty() && same_address(ip, p))
        };
        let mut shown = self.servers().iter().any(|s| same_address(&s.ip, address));
        let mut changed = false;
        self.entries.retain_mut(|e| {
            if !old(&text(e, "ip")) {
                return true;
            }
            changed = true;
            if !hidden(e) {
                if shown {
                    return false;
                }
                shown = true;
            }
            e.insert(key("ip"), Tag::String(address.into()));
            true
        });
        changed
    }
}

/// Puts the official server at `address` first in the list in `home`, once: not again after the
/// player removes it, and not when the list already has its address. When the server moved (the
/// address differs from the one offered last), the entries for its `previous` addresses move to
/// the new one, once, and it is not offered again to a player who removed it. A list that can't be
/// read is left alone; an empty `address` changes nothing.
pub fn add_official(home: &Path, address: &str, previous: &[String]) -> Result<(), String> {
    let address = address.trim();
    if address.is_empty() {
        return Ok(());
    }
    let marker = home.join(OFFICIAL_MARKER);
    // A marker that is there but can't be read: offered already, at an address not known.
    let offered = match fs::read_to_string(&marker) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => Some(String::new()),
    };
    if offered.as_deref().is_some_and(|a| same_address(a, address)) {
        return Ok(());
    }
    let mut list = List::read(home).ok_or("The server list can't be read.")?;
    let mut changed = list.move_official(address, previous);
    if offered.is_none() && !list.servers().iter().any(|s| same_address(&s.ip, address)) {
        list.entries.insert(0, entry(OFFICIAL_NAME, address));
        changed = true;
    }
    if changed {
        list.save(home)?;
    }
    fs::write(marker, format!("{address}\n")).map_err(|e| e.to_string())
}

/// The shown server a command names: its position from 1, its name (any case) or its address.
pub fn find(list: &List, query: &str) -> Result<(usize, Server), String> {
    let servers = list.servers();
    let q = query.trim();
    q.parse::<usize>()
        .ok()
        .filter(|n| (1..=servers.len()).contains(n))
        .map(|n| n - 1)
        .or_else(|| servers.iter().position(|s| s.name.eq_ignore_ascii_case(q)))
        .or_else(|| servers.iter().position(|s| s.ip.eq_ignore_ascii_case(q)))
        .map(|i| (i, servers[i].clone()))
        .ok_or_else(|| format!("No server {q} in the list"))
}

#[cfg(test)]
#[path = "server_list/tests.rs"]
mod tests;
