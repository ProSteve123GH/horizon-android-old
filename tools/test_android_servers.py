#!/usr/bin/env python3
"""Exercise the Android Server Manager without Android, JNI or the game."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    root = ROOT / '.local/android/server-manager-tests'
    (root / 'src').mkdir(parents=True, exist_ok=True)
    (root / 'Cargo.toml').write_text('''[package]
name = "android-server-manager-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
base64 = "0.22"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
horizon-world = { path = "../../../crates/horizon-world" }
horizon-telemetry = { path = "../../../crates/horizon-telemetry", default-features = false }
''', encoding='utf-8')
    (root / 'src/main.rs').write_text(RUST, encoding='utf-8')
    subprocess.run(['cargo', 'run', '--quiet', '--manifest-path', str(root / 'Cargo.toml')],
                   cwd=ROOT, check=True)


RUST = r'''
#![allow(dead_code)]
#[path = "../../../../crates/horizon-client/src/android/servers.rs"]
mod servers;
#[path = "../../../../launcher/src/server_list.rs"]
mod server_list;

use base64::Engine;
use horizon_world::nbt::{self, Compound, JavaString, Limits, NamedTag, Tag};
use serde_json::{Value, json};
use server_list::List;
use servers::{Address, Manager, Ping, MAX_GROUP, MAX_ICON_BYTES, MAX_NAME};
use std::fs;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "horizon-android-servers-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn s(value: &str) -> Tag {
    Tag::String(value.into())
}

fn raw_entries(home: &Path) -> Vec<Compound> {
    let bytes = fs::read(home.join("servers.dat")).unwrap();
    let root = nbt::read_named(&bytes, Limits::default()).unwrap().0;
    match root.tag.get("servers") {
        Some(Tag::List(list)) => list
            .iter()
            .filter_map(|tag| match tag {
                Tag::Compound(entry) => Some(entry.clone()),
                _ => None,
            })
            .collect(),
        other => panic!("servers.dat has no server list: {other:?}"),
    }
}

fn raw_text(entry: &Compound, key: &str) -> String {
    match entry.get(&JavaString::from(key)) {
        Some(Tag::String(text)) => text.to_string_lossy(),
        _ => String::new(),
    }
}

fn write_raw(home: &Path, entries: Vec<Tag>) {
    let root = NamedTag {
        name: "".into(),
        tag: Tag::compound([("servers", Tag::List(entries))]),
    };
    let bytes = nbt::write_named(&root, Limits::default()).unwrap();
    fs::write(home.join("servers.dat"), bytes).unwrap();
}

fn shown(home: &Path) -> Vec<(String, String)> {
    List::load(home)
        .servers()
        .into_iter()
        .map(|server| (server.name, server.ip))
        .collect()
}

fn state(manager: &Manager) -> Value {
    manager.state("Test instance")
}

fn item(value: &Value, index: usize) -> Value {
    value["servers"][index].clone()
}

fn names(value: &Value) -> Vec<String> {
    value["servers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|server| server["name"].as_str().unwrap().to_owned())
        .collect()
}

fn meta(home: &Path) -> Value {
    serde_json::from_slice(&fs::read(home.join(servers::META_FILE)).unwrap()).unwrap()
}

fn address_validation() {
    let cases = [
        ("play.example.com", "play.example.com", 25565),
        ("play.example.com:25570", "play.example.com", 25570),
        ("192.0.2.1", "192.0.2.1", 25565),
        ("192.0.2.1:25565", "192.0.2.1", 25565),
        ("[2001:db8::1]", "2001:db8::1", 25565),
        ("[2001:db8::1]:25570", "2001:db8::1", 25570),
        ("2001:db8::1", "2001:db8::1", 25565),
        ("localhost", "localhost", 25565),
        ("PLAY.Example.COM:00080", "PLAY.Example.COM", 80),
    ];
    for (text, host, port) in cases {
        let address = servers::validate_address(text).unwrap_or_else(|error| {
            panic!("{text:?} should be valid: {error}")
        });
        assert_eq!(address, Address { host: host.into(), port }, "{text:?}");
    }
    assert_eq!(
        servers::validate_address("[::1]:25570").unwrap().display(),
        "[::1]:25570"
    );
    assert_eq!(
        servers::validate_address("2001:db8::1").unwrap().display(),
        "[2001:db8::1]:25565"
    );
    for bad in [
        "",
        "   ",
        "host name",
        "host\tname",
        "host:0",
        "host:65536",
        "host:-1",
        "host:abc",
        "host:",
        "host: 25565",
        "https://example.com",
        "example.com/path",
        "example.com?x",
        "user@example.com",
        "bad..host",
        "-bad.example",
        "bad-.example",
        "1.2.3",
        "999.1.1.1",
        "[::1",
        "[]",
        "[]:25565",
        "[::1]extra",
        "::gg",
        "fe80::1%wlan0",
    ] {
        assert!(
            servers::validate_address(bad).is_err(),
            "{bad:?} should be rejected"
        );
    }
}

fn adding_validating_and_persisting() {
    let home = scratch("add");
    let mut manager = Manager::load(&home);
    assert_eq!(state(&manager)["servers"].as_array().unwrap().len(), 0);

    manager
        .add("  Friends  ", " play.example:25570 ", " Friends ", true)
        .unwrap();
    let before = state(&manager);
    assert_eq!(before["instance"].as_str(), Some("Test instance"));
    let first = item(&before, 0);
    assert_eq!(first["name"].as_str(), Some("Friends"));
    assert_eq!(first["address"].as_str(), Some("play.example:25570"));
    assert_eq!(first["host"].as_str(), Some("play.example"));
    assert_eq!(first["port"].as_u64(), Some(25570));
    assert_eq!(first["display_address"].as_str(), Some("play.example:25570"));
    assert_eq!(first["favorite"].as_bool(), Some(true));
    assert_eq!(first["group"].as_str(), Some("Friends"));
    assert!(first["icon"].is_null());
    assert_eq!(before["groups"], json!(["Friends"]));
    assert_eq!(shown(&home), [("Friends".into(), "play.example:25570".into())]);
    assert!(home.join(servers::META_FILE).is_file());

    manager.add("SMP", "smp.example", "SMP", false).unwrap();
    manager
        .add("Favourite", "fav.example", "Friends", true)
        .unwrap();
    let after = state(&manager);
    assert_eq!(names(&after), ["Friends", "SMP", "Favourite"]);
    assert_eq!(after["groups"], json!(["Friends", "SMP"]));
    assert_eq!(item(&after, 2)["port"].as_u64(), Some(25565));
    assert_eq!(item(&after, 2)["display_address"].as_str(), Some("fav.example:25565"));

    // Reopening reads both files and keeps the metadata beside the game's list.
    let reopened = Manager::load(&home);
    assert_eq!(state(&reopened), after);
    assert_eq!(meta(&home)["records"].as_array().unwrap().len(), 3);

    // Invalid input changes neither file nor the in-memory view.
    let snapshot = state(&manager);
    for bad in [
        "",
        "   ",
        "not a host",
        "https://example.com",
        "example.com:0",
        "example.com:65536",
        "example.com:abc",
        "[::1",
        "[]",
        "1.2.3",
    ] {
        assert!(
            manager.add("Bad", bad, "", false).is_err(),
            "{bad:?} should be rejected"
        );
    }
    assert!(manager.add(&"n".repeat(MAX_NAME + 1), "long-name.example", "", false).is_err());
    assert!(manager.add("Long group", "long-group.example", &"g".repeat(MAX_GROUP + 1), false).is_err());
    assert!(manager.add("Newline group", "newline.example", "bad\nline", false).is_err());
    assert_eq!(state(&manager), snapshot);

    manager
        .add(&"n".repeat(MAX_NAME), "long-name.example", &"g".repeat(MAX_GROUP), false)
        .unwrap();
    manager.add("", "blank.example", "", false).unwrap();
    let last = item(&state(&manager), 4);
    assert_eq!(last["name"].as_str(), Some("Minecraft Server"));
    assert_eq!(last["group"].as_str(), Some(""));
}

fn editing_keeps_unknown_fields_and_replaces_a_stale_icon() {
    let home = scratch("edit");
    let png = [0x89, b'P', b'N', b'G', 1, 2, 3, 4];
    let icon = base64::engine::general_purpose::STANDARD.encode(png);
    write_raw(
        &home,
        vec![
            Tag::compound([
                ("name", s("Friends")),
                ("ip", s("friends.example:25570")),
                ("icon", s(&icon)),
                ("acceptTextures", Tag::Byte(1)),
                ("acceptedCodeOfConduct", Tag::Int(-12345)),
                ("hidden", Tag::Byte(0)),
            ]),
            Tag::compound([
                ("name", s("Direct")),
                ("ip", s("direct.example")),
                ("hidden", Tag::Byte(1)),
            ]),
        ],
    );
    let mut manager = Manager::load(&home);
    let before = state(&manager);
    assert_eq!(item(&before, 0)["icon"].as_str(), Some(icon.as_str()));
    assert_eq!(before["servers"].as_array().unwrap().len(), 1);

    manager
        .update(0, "  Friends SMP  ", " friends.example:25570 ", "SMP", true)
        .unwrap();
    let edited = state(&manager);
    let first = item(&edited, 0);
    assert_eq!(first["name"].as_str(), Some("Friends SMP"));
    assert_eq!(first["icon"].as_str(), Some(icon.as_str()));
    assert_eq!(first["favorite"].as_bool(), Some(true));
    assert_eq!(first["group"].as_str(), Some("SMP"));
    let saved = raw_entries(&home);
    assert_eq!(saved.len(), 2, "the hidden Direct Connection entry stays");
    assert_eq!(raw_text(&saved[0], "name"), "Friends SMP");
    assert_eq!(raw_text(&saved[0], "ip"), "friends.example:25570");
    assert_eq!(raw_text(&saved[0], "icon"), icon);
    assert!(matches!(
        saved[0].get(&JavaString::from("acceptTextures")),
        Some(Tag::Byte(1))
    ));
    assert!(matches!(
        saved[0].get(&JavaString::from("acceptedCodeOfConduct")),
        Some(Tag::Int(-12345))
    ));
    assert!(!raw_text(&saved[1], "hidden").is_empty() || matches!(
        saved[1].get(&JavaString::from("hidden")),
        Some(Tag::Byte(1))
    ));

    // A changed address invalidates the picture; the fields the launcher does not know stay.
    manager
        .update(0, "Friends SMP", "friends.example:25571", "SMP", true)
        .unwrap();
    assert!(item(&state(&manager), 0)["icon"].is_null());
    let saved = raw_entries(&home);
    assert!(saved[0].get(&JavaString::from("icon")).is_none());
    assert!(matches!(
        saved[0].get(&JavaString::from("acceptTextures")),
        Some(Tag::Byte(1))
    ));
    assert!(matches!(
        saved[0].get(&JavaString::from("acceptedCodeOfConduct")),
        Some(Tag::Int(-12345))
    ));

    let before_bad = state(&manager);
    assert!(manager.update(0, "Bad", "bad address", "", false).is_err());
    assert!(manager.update(0, "Bad", "example.com:0", "", false).is_err());
    assert!(manager.update(99, "Missing", "missing.example", "", false).is_err());
    assert_eq!(state(&manager), before_bad);
}

fn invalid_stored_addresses_and_large_icons_are_harmless() {
    let home = scratch("stored");
    let huge = vec![7u8; MAX_ICON_BYTES + 1];
    let huge_icon = base64::engine::general_purpose::STANDARD.encode(huge);
    write_raw(
        &home,
        vec![
            Tag::compound([
                ("name", s("Old")),
                ("ip", s("not a host")),
                ("hidden", Tag::Byte(0)),
            ]),
            Tag::compound([
                ("name", s("Huge icon")),
                ("ip", s("huge.example")),
                ("icon", s(&huge_icon)),
                ("hidden", Tag::Byte(0)),
            ]),
        ],
    );
    let manager = Manager::load(&home);
    let state = state(&manager);
    assert_eq!(item(&state, 0)["address"].as_str(), Some("not a host"));
    assert!(item(&state, 0)["port"].is_null());
    assert!(item(&state, 0)["display_address"].is_null());
    assert!(item(&state, 1)["icon"].is_null());
}

fn metadata_reconciliation_and_corruption() {
    let home = scratch("meta");
    let mut manager = Manager::load(&home);
    manager.add("A", "a.example:25565", "SMP", true).unwrap();
    manager.add("B", "b.example", "Other", false).unwrap();

    // The game renamed A without changing its address: the launcher-only data follows it.
    let mut list = List::load(&home);
    list.edit(0, "A renamed", "a.example").unwrap();
    list.save(&home).unwrap();
    let manager = Manager::load(&home);
    let reconciled = state(&manager);
    assert_eq!(item(&reconciled, 0)["name"].as_str(), Some("A renamed"));
    assert_eq!(item(&reconciled, 0)["favorite"].as_bool(), Some(true));
    assert_eq!(item(&reconciled, 0)["group"].as_str(), Some("SMP"));

    // Deleting a server in the game drops its old record on the next launcher action.
    let mut list = List::load(&home);
    list.remove(1).unwrap();
    list.save(&home).unwrap();
    let mut manager = Manager::load(&home);
    assert_eq!(names(&state(&manager)), ["A renamed"]);
    assert_eq!(state(&manager)["groups"], json!(["SMP"]));
    manager.toggle_favorite(0).unwrap();
    assert_eq!(meta(&home)["records"].as_array().unwrap().len(), 1);
    assert_eq!(meta(&home)["records"][0]["favorite"].as_bool(), Some(false));

    // A name is a unique fallback when the address changed too.
    let home = scratch("meta-name");
    write_raw(
        &home,
        vec![Tag::compound([
            ("name", s("Old")),
            ("ip", s("new.example")),
            ("hidden", Tag::Byte(0)),
        ])],
    );
    fs::write(
        home.join(servers::META_FILE),
        r#"{"version":1,"records":[{"name":"Old","ip":"old.example","favorite":true,"group":"G"},{"name":"Ghost","ip":"ghost.example","favorite":true,"group":"G"}]}"#,
    )
    .unwrap();
    let manager = Manager::load(&home);
    let matched = state(&manager);
    assert_eq!(item(&matched, 0)["favorite"].as_bool(), Some(true));
    assert_eq!(item(&matched, 0)["group"].as_str(), Some("G"));
    assert_eq!(matched["groups"], json!(["G"]));
    let mut manager = manager;
    manager.toggle_favorite(0).unwrap();
    let records = meta(&home)["records"].as_array().unwrap().clone();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["name"].as_str(), Some("Old"));

    // :25565 and no port mean the same address when matching records.
    let home = scratch("meta-port");
    write_raw(
        &home,
        vec![Tag::compound([
            ("name", s("Port")),
            ("ip", s("port.example:25565")),
            ("hidden", Tag::Byte(0)),
        ])],
    );
    fs::write(
        home.join(servers::META_FILE),
        r#"{"version":1,"records":[{"name":"Port","ip":"port.example","favorite":true,"group":""}]}"#,
    )
    .unwrap();
    assert_eq!(item(&state(&Manager::load(&home)), 0)["favorite"].as_bool(), Some(true));

    // Unreadable JSON is set aside, never fatal; the next favorite action writes a good file.
    let home = scratch("meta-corrupt");
    let mut manager = Manager::load(&home);
    manager.add("A", "a.example", "", true).unwrap();
    fs::write(home.join(servers::META_FILE), b"{not json").unwrap();
    let mut manager = Manager::load(&home);
    let clean = state(&manager);
    assert_eq!(item(&clean, 0)["favorite"].as_bool(), Some(false));
    assert!(home.join("android-servers.json.corrupt").is_file());
    assert_eq!(manager.toggle_favorite(0).unwrap(), true);
    assert_eq!(meta(&home)["records"][0]["favorite"].as_bool(), Some(true));
}

fn duplicating_moving_and_removing() {
    let home = scratch("order");
    write_raw(
        &home,
        vec![
            Tag::compound([
                ("name", s("A")),
                ("ip", s("a.example")),
                ("hidden", Tag::Byte(0)),
            ]),
            Tag::compound([
                ("name", s("B")),
                ("ip", s("b.example")),
                ("icon", s("AAAA")),
                ("hidden", Tag::Byte(0)),
            ]),
            Tag::compound([
                ("name", s("C")),
                ("ip", s("c.example")),
                ("hidden", Tag::Byte(0)),
            ]),
            Tag::compound([
                ("name", s("Direct")),
                ("ip", s("direct.example")),
                ("hidden", Tag::Byte(1)),
            ]),
        ],
    );
    let mut manager = Manager::load(&home);
    manager.update(0, "A", "a.example", "G", true).unwrap();
    manager.update(1, "B", "b.example", "G", false).unwrap();
    manager.update(2, "C", "c.example", "Other", false).unwrap();

    let copy = manager.duplicate(1).unwrap();
    assert_eq!(copy.name, "B (copy)");
    assert_eq!(copy.ip, "b.example");
    let duplicated = state(&manager);
    assert_eq!(names(&duplicated), ["A", "B", "B (copy)", "C"]);
    assert_eq!(item(&duplicated, 2)["group"].as_str(), Some("G"));
    assert_eq!(item(&duplicated, 3)["group"].as_str(), Some("Other"));

    manager.move_by(2, -1).unwrap();
    let moved = state(&manager);
    assert_eq!(names(&moved), ["A", "B (copy)", "B", "C"]);
    assert_eq!(item(&moved, 1)["group"].as_str(), Some("G"));
    manager.move_by(2, -1).unwrap();
    assert_eq!(names(&state(&manager)), ["A", "B", "B (copy)", "C"]);
    assert!(manager.move_by(0, -1).is_err());
    assert!(manager.move_by(3, 1).is_err());
    assert!(manager.move_by(9, 0).is_err());
    assert!(manager.duplicate(9).is_err());
    assert!(manager.toggle_favorite(9).is_err());
    assert!(manager.address(9).is_err());

    assert_eq!(manager.address(1).unwrap(), "b.example");
    assert_eq!(manager.addresses().len(), 4);
    let removed = manager.remove(0).unwrap();
    assert_eq!(removed.name, "A");
    assert_eq!(names(&state(&manager)), ["B", "B (copy)", "C"]);
    assert!(manager.remove(9).is_err());

    // The hidden entry is still last after every operation, and the file reopened matches.
    let saved = raw_entries(&home);
    assert_eq!(saved.len(), 4);
    assert!(matches!(
        saved.last().unwrap().get(&JavaString::from("hidden")),
        Some(Tag::Byte(1))
    ));
    assert_eq!(
        shown(&home)
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["B", "B (copy)", "C"]
    );
    assert_eq!(meta(&home)["records"].as_array().unwrap().len(), 3);
}

fn ping_results_are_honest() {
    let pending = Ping::Pending;
    assert_eq!(pending.to_json(), json!({"state": "pending"}));

    let online = Ping::from_result(Ok((
        json!({"players": {"online": 3, "max": 10}, "description": "Hi"}),
        42,
    )));
    assert_eq!(
        online.to_json(),
        json!({
            "state": "online",
            "players": {"online": 3, "max": 10},
            "latency_ms": 42
        })
    );
    let no_players = Ping::from_result(Ok((json!({"description": "Hi"}), 0)));
    assert_eq!(
        no_players.to_json(),
        json!({"state": "online", "players": null, "latency_ms": 0})
    );

    let offline = Ping::from_result(Err("connection refused\nretry later".into()));
    let value = offline.to_json();
    assert_eq!(value["state"].as_str(), Some("offline"));
    let message = value["error"].as_str().unwrap();
    assert!(message.contains("connection refused retry later"));
    assert!(!message.contains('\n'));

    let empty = Ping::from_result(Err(String::new()));
    assert_eq!(empty.to_json()["error"].as_str(), Some("No status reply."));
}

fn main() {
    address_validation();
    println!("ok address validation");
    adding_validating_and_persisting();
    println!("ok add, validation and persistence");
    editing_keeps_unknown_fields_and_replaces_a_stale_icon();
    println!("ok edit preserves unknown fields and icons");
    invalid_stored_addresses_and_large_icons_are_harmless();
    println!("ok invalid stored addresses and large icons");
    metadata_reconciliation_and_corruption();
    println!("ok favorites, groups and metadata reconciliation");
    duplicating_moving_and_removing();
    println!("ok duplicate, move, remove and hidden entries");
    ping_results_are_honest();
    println!("ok status results");
}
'''


if __name__ == '__main__':
    main()
