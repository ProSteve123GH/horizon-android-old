//! Horizon mods in the game (`docs/mods.md`): loading the ones turned on,
//! their tick, frame, chat and command hooks, their HUD panels and what
//! they print. With no mod on, `horizon_mods::Mods` holds no engine and
//! every hook here returns after one check.
//!
//! Code mods run only in builds with the `mods` feature (release builds):
//! without it the game links no wasmtime, and every hook does nothing.
//! Content mods (`content_root`) load in every build.
use super::Game;
use horizon_client::{
    command::{Dispatcher, vanilla},
    extras_hud::Corners,
    hud::HudBatch,
    lang,
    mod_list::{self, Installed},
    text::{self, Text},
};
use std::{path::PathBuf, time::Duration};
#[cfg(feature = "mods")]
use {
    super::Screen,
    super::{lan::LanHost, server_mods::ModEvent},
    horizon_client::{
        extras::Corner, extras_hud, items::ItemStack, keybinds, options_ui::Page, ui,
    },
    horizon_mods::{ModKey, Mods, ServerPlayer, Snapshot, UiScreen, UiWidget},
    std::time::Instant,
    winit::{event::MouseButton, keyboard::KeyCode},
};

#[derive(Default)]
// Code-mod state; the WebAssembly runtime is not wired in this build.
#[allow(dead_code)]
pub(super) struct ModsState {
    #[cfg(feature = "mods")]
    pub(super) mods: Mods,
    /// Capability `input`: a claimed item's use while the button is down.
    #[cfg(feature = "mods")]
    pub(super) using: Option<super::mod_input::ModUse>,
    /// Capability `render`'s camera shakes.
    #[cfg(feature = "mods")]
    pub(super) shake: super::mod_render::Shake,
    /// `--dev-mod PATH`: always on, reloaded when rebuilt.
    pub(super) dev: Vec<PathBuf>,
    /// When dev mods were last checked for a rebuild.
    #[cfg(feature = "mods")]
    checked: Option<Instant>,
    /// The chat's command tree while a running mod owns commands.
    #[cfg(feature = "mods")]
    tree: Option<Dispatcher>,
    /// A server mod runs here: what players do is queued for it.
    #[cfg(feature = "mods")]
    pub(super) server: bool,
    /// The players server mods saw last tick, for joins and leaves.
    #[cfg(feature = "mods")]
    pub(super) players: Vec<ServerPlayer>,
    /// What players did this tick, for server mods at its end.
    #[cfg(feature = "mods")]
    pub(super) events: Vec<ModEvent>,
    /// A running server mod has capability `world`: its calls get the
    /// levels' chunks to read.
    #[cfg(feature = "mods")]
    pub(super) world: bool,
    /// Blocks server mods set, by dimension, made before that level's next
    /// tick.
    #[cfg(feature = "mods")]
    pub(super) block_writes: Vec<(String, horizon_mods::BlockChange)>,
    /// `command.run` lines for this window's own world, run on the next tick:
    /// run at once, their chat lines would reach the mods while their output
    /// is still being shown, and a mod answering its own lines would recurse.
    #[cfg(feature = "mods")]
    commands: Vec<String>,
    /// `command.run-quiet` batches for this window's own world, by mod id,
    /// run on the next tick as `commands` are.
    #[cfg(feature = "mods")]
    quiet: Vec<(String, Vec<String>)>,
    /// The server's command tree with the running mods' commands added.
    #[cfg(feature = "mods")]
    remote_tree: Option<Dispatcher>,
    /// A `ui` mod's screen or settings page while it is up.
    #[cfg(feature = "mods")]
    screen: Option<ModScreen>,
    /// The HUD font's advances for `render.text-width`, made once.
    #[cfg(feature = "mods")]
    font_widths: std::sync::Arc<[f32]>,
    /// Capability `recipes`'s tables, made once a mod reads them.
    #[cfg(feature = "mods")]
    pub(super) recipes: Option<std::sync::Arc<horizon_mods::Recipes>>,
    /// Extras that are official mods now, being installed.
    #[cfg(feature = "mods")]
    pub(super) moved: super::moved_extras::MovedState,
    /// Playing at the last frame, for `mod_input_frame`.
    #[cfg(feature = "mods")]
    pub(super) playing: bool,
    /// The WebAssembly mods found that no longer run, for the one-time notice.
    stopped: Vec<String>,
    /// Browse mods' worker (`mod_browser`).
    pub(super) jobs: super::mod_browser::Jobs,
    /// The content mods the loaded resources were built with: a switch the
    /// game can apply live moves it to the player's choice (`switch_mods`).
    pub(super) applied: Option<std::collections::BTreeSet<String>>,
    /// A live switch's resource reload is running: when it applied, the server
    /// reloads its data from the new root. Its start, for the log.
    pub(super) live: Option<std::time::Instant>,
    /// The content mods installed, and those of them with only assets and
    /// data (`mod_content::live_switch`): they switch with a world open.
    content_ids: std::collections::BTreeSet<String>,
    live_ids: std::collections::BTreeSet<String>,
    /// The server's data comes from a root a live switch built, not the one
    /// `res` was loaded from: the world tables are built from it even with no
    /// data pack (`build_world_tables`).
    pub(super) data_switched: bool,
}

/// The mappings `input.mapping` answers for, with the default key their
/// binding is translated to (none: a mouse button, attack or use).
#[cfg(feature = "mods")]
#[allow(dead_code)]
const MAPPINGS: [(&str, Option<winit::keyboard::KeyCode>); 9] = {
    use winit::keyboard::KeyCode as K;
    [
        ("key.forward", Some(K::KeyW)),
        ("key.left", Some(K::KeyA)),
        ("key.back", Some(K::KeyS)),
        ("key.right", Some(K::KeyD)),
        ("key.jump", Some(K::Space)),
        ("key.sneak", Some(K::ShiftLeft)),
        ("key.sprint", Some(K::ControlLeft)),
        ("key.attack", None),
        ("key.use", None),
    ]
};
/// Characters `render.text-width` knows exactly: U+0000 to U+024F.
#[cfg(feature = "mods")]
#[allow(dead_code)]
const FONT_CHARS: u32 = 0x250;

/// A `ui` mod's own screen, or its settings page from Options > Mods: the
/// widgets the mod pushed, laid out and drawn with the menus' own widgets.
#[cfg(feature = "mods")]
struct ModScreen {
    /// The running mod's index in `mods.mods.entries()`.
    owner: usize,
    /// Opened from Options > Mods: Done and Esc return there.
    settings: bool,
    /// What the mod's screen replaced (Playing, Chat, ...), to restore.
    under: Screen,
    /// One scroll per `list` widget, in widget order.
    scrolls: Vec<ui::Scroll>,
    /// A key row waiting for the key to bind: the mod key's name.
    binding: Option<String>,
}
impl ModsState {
    /// The chat's command tree: vanilla's, plus the running mods' commands.
    pub(super) fn dispatcher(&self) -> &Dispatcher {
        #[cfg(feature = "mods")]
        if let Some(tree) = &self.tree {
            return tree;
        }
        vanilla::game_dispatcher()
    }
    /// The server's tree plus the mods' commands, when a mod owns any.
    pub(super) fn remote_dispatcher(&self) -> Option<&Dispatcher> {
        #[cfg(feature = "mods")]
        if let Some(tree) = &self.remote_tree {
            return Some(tree);
        }
        None
    }
    /// Whether a mod runs: the chat keeps its lines for them.
    pub(super) fn running(&self) -> bool {
        #[cfg(feature = "mods")]
        if self.mods.is_running() {
            return true;
        }
        false
    }
    /// Whether a mod draws a HUD panel or a corner panel.
    pub(super) fn has_panels(&self) -> bool {
        #[cfg(feature = "mods")]
        if self.mods.panels().next().is_some() || self.mods.corner_panels().next().is_some() {
            return true;
        }
        false
    }
}
/// How often dev mods are checked for a rebuild.
#[cfg(feature = "mods")]
#[allow(dead_code)]
const RELOAD_CHECK: Duration = Duration::from_secs(1);

/// A vanilla command name: mods may not take it.
#[cfg(feature = "mods")]
#[allow(dead_code)]
fn vanilla_command(name: &str) -> bool {
    let d = vanilla::game_dispatcher();
    d.nodes[0]
        .children
        .iter()
        .any(|&c| d.nodes[c].name() == name)
}

/// Freeze native component IDs before reading content or opening a connection.
/// With no mod registering any, nothing is installed, which every reader
/// treats as no custom components, and `registries.json` isn't read.
pub(super) fn register_components(root: &std::path::Path) -> horizon_client::Result<()> {
    let mut names = Vec::new();
    Game::native_component_types(&mut names);
    if names.is_empty() {
        return Ok(());
    }
    let path =
        horizon_protocol::registry::Builtin::find(root).ok_or("registries.json not found")?;
    let builtin = horizon_protocol::registry::Builtin::load(&path)?;
    let types = builtin
        .get("minecraft:data_component_type")
        .ok_or("missing data component registry")?;
    let first = i32::try_from(types.names.len())?;
    horizon_protocol::item::custom::install(first, names)
        .map_err(|error| format!("invalid native component registration: {error}"))?;
    Ok(())
}

/// The blocks and items native mods declare in code (`native_blocks`,
/// `native_items`): asked once, so every content root of this run, a pack
/// reload's included, has the same ones.
fn native_declared() -> &'static (
    Vec<horizon_mods::content::Block>,
    Vec<horizon_mods::content::Item>,
) {
    static DECLARED: std::sync::OnceLock<(
        Vec<horizon_mods::content::Block>,
        Vec<horizon_mods::content::Item>,
    )> = std::sync::OnceLock::new();
    DECLARED.get_or_init(|| {
        let (mut blocks, mut items) = (Vec::new(), Vec::new());
        Game::native_blocks(&mut blocks);
        Game::native_items(&mut items);
        (blocks, items)
    })
}
/// The native mods' standalone models (`native_standalone_models`), set once
/// before the resources first load.
pub(super) fn register_standalone_models() {
    let mut names = Vec::new();
    Game::native_standalone_models(&mut names);
    horizon_client::models::register_standalone(names);
}
/// The native mods' entity model layers (`native_layer_definitions`), kept
/// once at start for `native_bake_layer`.
pub(super) fn register_layer_definitions() {
    let mut layers = Vec::new();
    Game::native_layer_definitions(&mut layers);
    horizon_client::mobs::model_layers::register(layers);
}

/// `root` with the content mods that are on layered over it
/// (docs/modpacks.md): their blocks, items, data and assets. Read once at
/// start, before the resources load; a mod whose content is refused is left
/// out here and its row on the Mods page says why.
pub(super) fn content_root(
    root: &std::path::Path,
    enabled: &std::collections::BTreeSet<String>,
    disabled: &std::collections::BTreeSet<String>,
    dev: &[std::path::PathBuf],
) -> std::path::PathBuf {
    let here = std::env::current_dir().unwrap_or_default();
    let version = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_owned()));
    content_root_at(root, enabled, disabled, dev, &here, version.as_deref())
}

fn content_root_at(
    root: &std::path::Path,
    enabled: &std::collections::BTreeSet<String>,
    disabled: &std::collections::BTreeSet<String>,
    dev: &[std::path::PathBuf],
    here: &std::path::Path,
    version: Option<&std::path::Path>,
) -> std::path::PathBuf {
    use horizon_client::mod_content::{self, ContentMod};
    use horizon_mods::{Capability, Package};
    let _step = horizon_client::spans::startup::step("mods.content_root");
    let native = version
        .map(|p| horizon_mods::find(&p.join("native-content")))
        .unwrap_or_default();
    let native_ids: std::collections::BTreeSet<_> = native
        .iter()
        .filter_map(|path| Package::read(path).ok().map(|p| p.manifest.id))
        .collect();
    let mut paths = native.clone();
    paths.extend(horizon_mods::find(&here.join(mod_list::DIR)));
    paths.retain(|p| !dev.contains(p));
    paths.extend(dev.iter().cloned());
    let mut known = None;
    let mut mods: Vec<ContentMod> = Vec::new();
    // The first load's list stays (`set_local`): a reload does not hash the files again.
    let announce = !horizon_protocol::mod_list::is_set();
    let mut announced = Vec::new();
    let packages: Vec<(&PathBuf, Package)> = paths
        .iter()
        .filter_map(|path| Some((path, Package::read(path).ok()?)))
        .filter(|(path, package)| {
            let m = &package.manifest;
            // A legacy content copy kept for an older build cannot replace this build's
            // snapshot, including when the native mod is off.
            if !native.contains(path) && native_ids.contains(&m.id) && !dev.contains(path) {
                return false;
            }
            let on = if native.contains(path) {
                !disabled.contains(&m.id)
            } else {
                enabled.contains(&m.id)
            } || dev.contains(path);
            on && m.capabilities.contains(&Capability::Registries)
        })
        .collect();
    let read = read_contents(&packages);
    for ((path, package), content) in packages.iter().zip(read) {
        let m = &package.manifest;
        if mods.iter().any(|c| c.id == m.id) {
            continue;
        }
        let content = match content {
            Ok(content) => content,
            Err(e) => {
                log::warn!("Horizon: mod {} content not loaded: {e}", m.id);
                continue;
            }
        };
        let known = match &mut known {
            Some(types) => types,
            None => match mod_content::known_components(root) {
                Ok(types) => known.insert(types),
                Err(e) => {
                    log::warn!("Horizon: content mods not loaded: {e}");
                    register_mob_effects(&[]);
                    return root.to_owned();
                }
            },
        };
        let candidate = ContentMod {
            id: m.id.clone(),
            content,
        };
        match mod_content::accept_mod(&mut mods, candidate, known) {
            Ok(_) if announce => announced.push(announced_entry(m, path)),
            Ok(_) => {}
            Err(e) => log::warn!("Horizon: mod {} content not loaded: {e}", m.id),
        }
    }
    let (blocks, items) = native_declared();
    if !blocks.is_empty() || !items.is_empty() {
        if known.is_none() {
            match mod_content::known_components(root) {
                Ok(types) => known = Some(types),
                Err(e) => log::warn!("Horizon: native mods' blocks and items not loaded: {e}"),
            }
        }
        if let Some(known) = &known {
            for e in mod_content::accept_declared(&mut mods, blocks, items, known) {
                log::warn!("Horizon: {e}");
            }
            // A file item's remainder named a code-declared item that was refused: that mod
            // alone is left out, not every content mod through the overlay's check.
            for e in mod_content::drop_dangling_remainders(&mut mods) {
                log::warn!("Horizon: {e}");
            }
        }
    }
    let overlay = {
        let _step = horizon_client::spans::startup::step("mods.overlay");
        mod_content::resource_root(root, &mods, &here.join(".horizon-cache"))
    };
    match overlay {
        Ok(out) => {
            if announce {
                horizon_protocol::mod_list::set_local(announced);
            }
            if out != root {
                log::info!(
                    "Horizon: content from {} mod(s) in {}",
                    mods.len(),
                    out.display()
                );
            }
            register_mob_effects(&mods);
            register_entity_types(&mods);
            register_menu_types(&mods);
            out
        }
        Err(e) => {
            log::warn!("Horizon: content mods not loaded: {e}");
            register_mob_effects(&[]);
            register_entity_types(&[]);
            register_menu_types(&[]);
            root.to_owned()
        }
    }
}

/// Each package's content (`Content::read_declaring`, with the item ids its native code
/// declares), in `packages`' order. Listing a mod folder and reading its declarations wait on
/// the file system, file by file (about 3.5 s for a large modpack's 150k files on one thread), so
/// the mods are read on threads at once.
fn read_contents(
    packages: &[(&PathBuf, horizon_mods::Package)],
) -> Vec<Result<horizon_mods::Content, String>> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    // Asked here once: native mods' hooks run on this thread.
    let native_items = &native_declared().1;
    let read = |(path, package): &(&PathBuf, horizon_mods::Package)| {
        let m = &package.manifest;
        // The items this mod's native code declares may be a file item's crafting remainder.
        let declared: Vec<String> = native_items
            .iter()
            .filter(|i| i.id.split_once(':').is_some_and(|(ns, _)| ns == m.id))
            .map(|i| i.id.clone())
            .collect();
        let content = horizon_mods::Content::read_declaring(path, m, &declared)?;
        // Read as `Content::read` reads it: `start_mods` need not read it again.
        if declared.is_empty() {
            remember_verdict(path, &content);
        }
        Ok(content)
    };
    let next = AtomicUsize::new(0);
    let threads = horizon_threads::worker_count().clamp(1, packages.len().max(1));
    let mut done: Vec<_> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(package) = packages.get(i) else {
                            return done;
                        };
                        done.push((i, read(package)));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    });
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, read)| read).collect()
}

/// A loaded mod as a server tells joining clients (`horizon_protocol::mod_list`): its manifest's
/// id, version and side, and the SHA-256 of its `.hmod` (none for a folder).
fn announced_entry(
    m: &horizon_mods::Manifest,
    path: &std::path::Path,
) -> horizon_protocol::mod_list::Entry {
    use horizon_mods::Side;
    use horizon_protocol::mod_list;
    use sha2::{Digest, Sha256};
    let sha256 = if path.is_file() {
        std::fs::read(path)
            .ok()
            .map(|bytes| Sha256::digest(bytes).into())
    } else {
        None
    };
    mod_list::Entry {
        id: m.id.clone(),
        version: m.version.clone(),
        sha256,
        side: match m.side {
            Side::Both => mod_list::Side::Both,
            Side::Server => mod_list::Side::Server,
            Side::Client => mod_list::Side::Client,
        },
    }
}

/// The mods' mob effects join vanilla's (`brewing::effect`), once the root
/// holding their network ids is built. The startup root's effects, none
/// included, stay until restart: a pack reload after a Mods switch builds a
/// root with others.
fn register_mob_effects(mods: &[horizon_client::mod_content::ContentMod]) {
    use horizon_client::brewing::{self, Category};
    use horizon_mods::content::EffectCategory;
    let effects = horizon_client::mod_content::mob_effects(mods);
    let ids: Vec<String> = effects.iter().map(|e| e.id.clone()).collect();
    let set = brewing::register_mod_effects(effects.into_iter().map(|e| {
        let category = match e.category {
            EffectCategory::Beneficial => Category::Beneficial,
            EffectCategory::Harmful => Category::Harmful,
            EffectCategory::Neutral => Category::Neutral,
        };
        (e.id.clone(), e.color, category)
    }));
    if !set
        && brewing::mod_effects()
            .iter()
            .map(|e| e.id)
            .ne(ids.iter().map(String::as_str))
    {
        log::warn!("Horizon: the mods' mob effects changed; restart the game to use them");
    }
}

/// A build without the `mods` feature: no mod runs.
#[cfg(not(feature = "mods"))]
impl Game<'_> {
    pub(super) fn merge_remote_tree(&mut self) {}
    pub(super) fn sync_mods(&mut self) {}
    pub(super) fn tick_mods(&mut self) {}
    pub(super) fn mods_frame(&mut self, _elapsed: Duration) {}
    pub(super) fn mod_command(&mut self, _command: &str) -> bool {
        false
    }
    pub(super) fn mods_chat(&mut self, _line: &str) {}
    pub(super) fn draw_mod_panels(&self, _batch: &mut HudBatch, _corners: &mut Corners) {}
    pub(super) fn open_mod_screen(&mut self, _owner: usize, _settings: bool) {}
    pub(super) fn draw_mod_screen(&mut self, _batch: &mut HudBatch) {}
    pub(super) fn mod_screen_click(&mut self, _button: winit::event::MouseButton) {}
    #[allow(dead_code)]
    pub(super) fn mod_screen_key(&mut self, _key: winit::keyboard::KeyCode) -> bool {
        false
    }
    pub(super) fn mod_screen_scroll(&mut self, _delta: f32) {}
}
/// the mods' thrown entity types, as their mob effects: set once, from the
/// startup root. Living types are mobs, loaded with vanilla's from the
/// content root's `entity-types.json`.
fn register_entity_types(mods: &[horizon_client::mod_content::ContentMod]) {
    use horizon_client::projectiles;
    let mut types = horizon_client::mod_content::entity_types(mods);
    types.retain(|t| t.item.is_some());
    let set = projectiles::register_thrown_types(
        types
            .iter()
            .filter_map(|t| Some((t.id.clone(), t.item.clone()?, t.damage))),
    );
    if !set
        && projectiles::thrown_types()
            .iter()
            .map(|t| t.id)
            .ne(types.iter().map(|t| t.id.as_str()))
    {
        log::warn!("Horizon: the mods' entity types changed; restart the game to use them");
    }
}

/// the mods' menu types, as their entity types: set once, from the startup
/// root.
fn register_menu_types(mods: &[horizon_client::mod_content::ContentMod]) {
    use horizon_client::containers;
    let menus = horizon_client::mod_content::menu_types(mods);
    let set = containers::register_mod_menu_types(menus.iter().map(|m| {
        let results = m.results.iter().map(|&r| usize::from(r)).collect();
        let crafting = m.crafting.map(|c| containers::ModCrafting {
            grid: c.grid.into(),
            size: c.size.into(),
            result: c.result.into(),
        });
        let links = m.links.map(|l| containers::ModLinks {
            from: l.from.into(),
            count: l.count.into(),
        });
        (
            m.id.clone(),
            m.slots.into(),
            m.data_slots.into(),
            results,
            crafting,
            links,
        )
    }));
    if !set
        && containers::mod_menu_types()
            .iter()
            .map(|m| m.id)
            .ne(menus.iter().map(|m| m.id.as_str()))
    {
        log::warn!("Horizon: the mods' menu types changed; restart the game to use them");
    }
}

/// What the launcher writes beside a game it built with native mods: their ids under `mods`.
const BUILD_INFO: &str = "native-mods.json";

/// `native-mods.json`'s `mods` list is not empty: the game was built with mods.
fn build_lists_mods(built: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(built)
        .ok()
        .and_then(|info| info["mods"].as_array().map(|mods| !mods.is_empty()))
        .unwrap_or(false)
}

/// Whether the game playing was built with native mods: horizon-mixins applied bundles to its
/// source (`HORIZON_MIXIN_MODS`, set by the build script from the record it leaves, as with a mod
/// author's own build that has no [`BUILD_INFO`]), trait mods are installed, or the launcher wrote
/// their ids into [`BUILD_INFO`] beside the executable. A release or Nightly has none of them.
pub(super) fn built_with_native_mods() -> bool {
    option_env!("HORIZON_MIXIN_MODS").is_some_and(|mods| !mods.is_empty())
        || !crate::mod_api::mods().is_empty()
        || std::env::current_exe()
            .ok()
            .and_then(|exe| std::fs::read_to_string(exe.with_file_name(BUILD_INFO)).ok())
            .is_some_and(|built| build_lists_mods(&built))
}

/// The native mods the launcher built this game with, `id version, id version`, for the crash
/// report (`built`: its [`BUILD_INFO`]). None for a release or Nightly, which have no mods.
fn native_mods_detail(built: &str) -> Option<String> {
    let info: serde_json::Value = serde_json::from_str(built).unwrap_or_default();
    let list = info["mods"]
        .as_array()?
        .iter()
        .map(|m| {
            let text = |key: &str| m[key].as_str().unwrap_or("?").to_owned();
            format!("{} {}", text("id"), text("version"))
        })
        .collect::<Vec<_>>()
        .join(", ");
    (!list.is_empty()).then_some(list)
}

/// Whether a content mod's files can switch with a world open (`mod_content::live_switch`);
/// `None` when its content can't be read.
type Verdict = Option<std::result::Result<(), &'static str>>;

/// Each mod's [`Verdict`] by its path and modification time. Reading a mod's content walks and
/// checks every file (seconds for a large pack), and a start read each mod three times: the
/// content root, then the window's and the server thread's `start_mods`.
/// shortcut: a file edited inside an unpacked mod folder keeps the folder's time; `--dev-mod`
/// folders are never remembered, a player's are replaced whole by the launcher.
static VERDICTS: std::sync::Mutex<
    std::collections::BTreeMap<(PathBuf, std::time::SystemTime), Verdict>,
> = std::sync::Mutex::new(std::collections::BTreeMap::new());

fn verdict_key(path: &std::path::Path) -> Option<(PathBuf, std::time::SystemTime)> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some((path.to_owned(), modified))
}

/// Remembers the verdict of `content`, read from `path` as [`horizon_mods::Content::read`] does.
fn remember_verdict(path: &std::path::Path, content: &horizon_mods::Content) {
    if let Some(key) = verdict_key(path) {
        let verdict = Some(horizon_client::mod_content::live_switch(content));
        VERDICTS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, verdict);
    }
}

/// The mod at `path`'s [`Verdict`], read once per version of it (never remembered for `dev`).
fn verdict(path: &std::path::Path, manifest: &horizon_mods::Manifest, dev: bool) -> Verdict {
    let key = if dev { None } else { verdict_key(path) };
    if let Some(key) = &key
        && let Some(verdict) = VERDICTS.lock().unwrap_or_else(|e| e.into_inner()).get(key)
    {
        return *verdict;
    }
    let verdict = horizon_mods::Content::read(path, manifest)
        .ok()
        .map(|content| horizon_client::mod_content::live_switch(&content));
    if let Some(key) = key {
        VERDICTS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, verdict);
    }
    verdict
}

/// Notes a content mod's id, and whether its files can switch with a world
/// open (`mod_content::live_switch`).
fn live_switch_of(
    path: &std::path::Path,
    package: &horizon_mods::Package,
    dev: bool,
    content_ids: &mut std::collections::BTreeSet<String>,
    live_ids: &mut std::collections::BTreeSet<String>,
) {
    let id = &package.manifest.id;
    content_ids.insert(id.clone());
    match verdict(path, &package.manifest, dev) {
        Some(Ok(())) if package.wasm.is_none() => {
            live_ids.insert(id.clone());
        }
        Some(Err(why)) => log::debug!("Horizon: mod {id} switches on a restart: {why}"),
        _ => {}
    }
}

/// A row for each native mod the launcher installed in `dir` (`<id>.hnmod`) that this game wasn't
/// built with (`built`: its [`BUILD_INFO`], empty for a release or Nightly) and that has no row
/// yet. The page said "No mods" before, and players copied the files into `horizon-mods` (#4770).
/// Its toggle is the `mod.<id>` line the launcher's next build follows.
fn not_built(dir: &std::path::Path, built: &str, rows: &[Installed]) -> Vec<Installed> {
    let info: serde_json::Value = serde_json::from_str(built).unwrap_or_default();
    let in_build = |id: &str| {
        info["mods"]
            .as_array()
            .is_some_and(|mods| mods.iter().any(|m| m["id"] == id))
    };
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.retain(|p| p.extension().is_some_and(|e| e == "hnmod"));
    paths.sort();
    let mut out: Vec<Installed> = Vec::new();
    for path in paths {
        let Ok(bundle) = horizon_mods::index::native::Bundle::read(&path) else {
            continue;
        };
        let m = bundle.manifest;
        let listed = rows.iter().chain(&out).any(|row| row.id == m.id);
        if !listed && !in_build(&m.id) {
            out.push(Installed {
                title: format!("{} {}", m.id, m.version),
                id: m.id,
                description: String::new(),
                problem: Some(mod_list::NOT_BUILT.to_owned()),
                dev: false,
                has_settings: false,
                native: None,
            });
        }
    }
    out
}

#[cfg(feature = "mods")]
impl Game<'_> {
    // The WebAssembly mod runtime's hooks: upstream stopped running
    // WebAssembly mods (their rows say so on the Mods page, `start_mods`), so
    // the game's calls reach these no-ops. The fork keeps the hooks because
    // its Android mod menu and server-mod events still call them.
    pub(super) fn tick_mods(&mut self) {}
    pub(super) fn mods_frame(&mut self, _elapsed: Duration) {}
    pub(super) fn mods_chat(&mut self, _line: &str) {}
    pub(super) fn show_mod_output(&mut self) {}
    pub(super) fn route_mod_output(&mut self, _host: Option<&mut LanHost>) {}
    pub(super) fn mod_snapshot_with(&mut self, _players: Vec<ServerPlayer>) -> Snapshot {
        Snapshot::default()
    }
    #[allow(dead_code)]
    pub(super) fn reload_dev_mods(&mut self) {}
    pub(super) fn merge_remote_tree(&mut self) {}
    pub(super) fn sync_mods(&mut self) {}
    pub(super) fn mod_command(&mut self, _command: &str) -> bool {
        false
    }
    /// A mod's line in the window's chat; not through system_message: a mod
    /// must not hear its own lines.
    /// Written by this client, so the Chat option doesn't hide it.
    #[allow(dead_code)]
    pub(super) fn add_mod_chat_line(&mut self, line: Text) {
        let focused = self.screen == Screen::Chat;
        self.ui.chat.add(
            &self.gui.atlas,
            line,
            None,
            horizon_client::chat::ChatSource::Client,
            self.ticks,
            focused,
        );
    }
    /// The mods' HUD panels, stacked with the extras in their corners.
    pub(super) fn draw_mod_panels(&self, batch: &mut HudBatch, corners: &mut Corners) {
        for panel in self.mods.mods.panels() {
            let corner = match panel.corner {
                horizon_mods::Corner::TopLeft => Corner::TopLeft,
                horizon_mods::Corner::TopRight => Corner::TopRight,
                horizon_mods::Corner::BottomLeft => Corner::BottomLeft,
                horizon_mods::Corner::BottomRight => Corner::BottomRight,
            };
            // F3 takes the top left, as it does from the extras.
            if self.debug && corner == Corner::TopLeft {
                continue;
            }
            let lines: Vec<(&str, u32)> = panel
                .lines
                .iter()
                .map(|l| (l.text.as_str(), l.color))
                .collect();
            extras_hud::draw_lines(batch, &self.gui.atlas, corners, corner, &lines);
        }
        // Capability `render`'s corner panels, placed the same way.
        for panel in self.mods.mods.corner_panels() {
            let corner = match panel.corner {
                horizon_mods::RenderCorner::TopLeft => Corner::TopLeft,
                horizon_mods::RenderCorner::TopRight => Corner::TopRight,
                horizon_mods::RenderCorner::BottomLeft => Corner::BottomLeft,
                horizon_mods::RenderCorner::BottomRight => Corner::BottomRight,
            };
            if self.debug && corner == Corner::TopLeft {
                continue;
            }
            let at = corners.place(corner, panel.width, panel.height);
            self.draw_shapes(batch, &panel.shapes, at);
        }
    }

    /// A `ui` screen's title and widgets as the mod last pushed them; None
    /// when the surface is gone (the mod stopped or closed it). Borrows
    /// `mods` only, so `self.mods.screen` may stay borrowed for the scrolls.
    fn mod_ui_view<'a>(mods: &'a Mods, ms: &ModScreen) -> Option<(&'a str, &'a [UiWidget])> {
        if ms.settings {
            let widgets = mods.settings_widgets(ms.owner);
            if widgets.is_empty() && mods.keys_of(ms.owner).is_empty() {
                return None;
            }
            let name = mods
                .entries()
                .get(ms.owner)
                .and_then(|e| e.manifest.as_ref())
                .map_or("Mod", |m| m.name.as_str());
            Some((name, widgets))
        } else {
            let s: &UiScreen = mods.screen(ms.owner)?;
            Some((s.title.as_str(), s.widgets.as_slice()))
        }
    }
    /// The keys a settings page lists first: the mod's own (`input`).
    fn mod_page_keys<'a>(mods: &'a Mods, ms: &ModScreen) -> &'a [ModKey] {
        if ms.settings {
            mods.keys_of(ms.owner)
        } else {
            &[]
        }
    }
    /// A key row's label as Key Binds shows one, "Zoom: Z", marked
    /// "> Zoom: Z <" in yellow while it waits for the new key.
    fn mod_key_label(k: &ModKey, waiting: bool) -> Text {
        let value = format!("{}: {}", k.label, keybinds::display_name(&k.key).plain());
        if waiting {
            Text::literal(format!("> {value} <")).with_color(text::YELLOW)
        } else {
            Text::literal(value)
        }
    }
    /// The player bound the key row that waited to `key`: the mod's key
    /// follows it now and at the next start (`horizon-mods.txt`).
    fn bind_mod_key(&mut self, key: String) {
        let Some(ms) = self.mods.screen.as_mut() else {
            return;
        };
        let Some(name) = ms.binding.take() else {
            return;
        };
        let owner = ms.owner;
        if !self.mods.mods.rebind(owner, &name, &key) {
            return;
        }
        let Some(id) = self
            .mods
            .mods
            .entries()
            .get(owner)
            .and_then(|e| e.manifest.as_ref())
            .map(|m| m.id.clone())
        else {
            return;
        };
        self.settings
            .mods
            .keys
            .entry(id)
            .or_default()
            .insert(name, key);
        self.settings.save(&Self::options_path());
    }
    /// A key while a mod screen is up: true when a key row waited for
    /// it (Key Binds: Escape binds none).
    #[allow(dead_code)]
    pub(super) fn mod_screen_key(&mut self, key: KeyCode) -> bool {
        if self
            .mods
            .screen
            .as_ref()
            .is_none_or(|ms| ms.binding.is_none())
        {
            return false;
        }
        let value = match key {
            KeyCode::Escape => None,
            k => keybinds::key_name(k),
        };
        self.bind_mod_key(value.unwrap_or("key.keyboard.unknown").to_owned());
        true
    }
    /// The layout the mod screen's widgets get at `h`: (widget, top,
    /// height), the column being `COLUMN` wide down the middle.
    fn mod_ui_layout(h: f32, top: f32, widgets: &[UiWidget]) -> Vec<(f32, f32)> {
        let mut out = Vec::with_capacity(widgets.len());
        let mut y = top;
        for w in widgets {
            let wh = match w {
                UiWidget::Text(_) => 12.0,
                UiWidget::Item(_) | UiWidget::Button(_) => 20.0,
                UiWidget::List(l) => (l.rows.len().min(6) as f32 * 12.0) + 8.0,
            };
            // Nothing may cover Done; a crowded mod loses what won't fit.
            if y + wh > h - 34.0 {
                break;
            }
            out.push((y, wh));
            y += wh + 6.0;
        }
        out
    }
    /// A mod pushes its screen (`ui.open-screen`), or Options > Mods opens
    /// its settings page (`settings`, the mod's `installed` index).
    pub(super) fn open_mod_screen(&mut self, owner: usize, settings: bool) {
        // Only the settings page is drawn at the title (Options > Mods); a
        // mod's own screen waits for a world.
        if self.title.is_some() && !settings {
            return;
        }
        if settings {
            self.mods.mods.ui_settings_opened(owner);
            let keys = self.mods.mods.keys_of(owner);
            if self.mods.mods.settings_widgets(owner).is_empty() && keys.is_empty() {
                self.show_mod_output();
                return;
            }
        }
        self.mods.screen = Some(ModScreen {
            owner,
            settings,
            under: self.screen,
            scrolls: Vec::new(),
            binding: None,
        });
        self.screen = Screen::Mod;
        if self.player.is_some() {
            self.capture(false);
        }
        self.show_mod_output();
    }
    /// Leaves the mod screen: back to Options for a settings page, to what
    /// it covered for a mod's own. `tell` is Esc/Done, heard as `closed`.
    pub(super) fn close_wasm_mod_screen(&mut self, tell: bool) {
        let Some(ms) = self.mods.screen.take() else {
            return;
        };
        if ms.settings {
            self.open_options_at(self.options_from_title, Page::Mods);
        } else {
            self.screen = ms.under;
            if self.screen == Screen::Playing && self.player.is_some() {
                self.capture(true);
            }
        }
        if tell {
            self.mods.mods.ui_closed(ms.owner);
            self.show_mod_output();
        }
    }
    /// The mod's screen or settings page: its title over widgets stacked
    /// down the middle, a Done button, Esc to leave (vanilla's menus).
    pub(super) fn draw_mod_screen(&mut self, batch: &mut HudBatch) {
        let Some(ms) = self.mods.screen.as_mut() else {
            // The screen was dropped while it was up: back to the world,
            // mouse held again like any return to Playing.
            self.screen = Screen::Playing;
            if self.player.is_some() {
                self.capture(true);
            }
            return;
        };
        let Some((title, widgets)) = Self::mod_ui_view(&self.mods.mods, ms) else {
            // The mod's surface is gone (it stopped or cleared it): leave
            // the screen like Done did, without `closed` â€” the mod ended it.
            self.close_wasm_mod_screen(false);
            return;
        };
        // Item icons draw in a second pass: `draw_item` needs `&mut self`,
        // so `atlas` may not stay borrowed across it.
        let mut items: Vec<(String, f32, f32)> = Vec::new();
        let atlas = &self.gui.atlas;
        let in_world = self.player.is_some();
        ui::menu_background(batch, atlas, 0.0, 0.0, batch.width, batch.height, in_world);
        ui::centered_text(
            batch,
            atlas,
            &Text::literal(title),
            0.0,
            15.0,
            batch.width,
            u32::MAX,
        );
        let cx = (batch.width / 2.0).floor();
        let x = cx - MOD_UI_COLUMN / 2.0;
        // A settings page starts with the mod's keys, as Key Binds rows.
        let keys = Self::mod_page_keys(&self.mods.mods, ms);
        for (i, k) in keys.iter().enumerate() {
            let wy = MOD_UI_TOP + i as f32 * MOD_UI_KEY_ROW;
            let hover = ui::hovered(self.cursor, x, wy, MOD_UI_COLUMN, 20.0);
            let waiting = ms.binding.as_deref() == Some(k.name.as_str());
            let label = Self::mod_key_label(k, waiting);
            ui::button(
                batch,
                atlas,
                x,
                wy,
                MOD_UI_COLUMN,
                20.0,
                &label,
                true,
                hover,
            );
        }
        let top = MOD_UI_TOP + keys.len() as f32 * MOD_UI_KEY_ROW;
        let layout = Self::mod_ui_layout(batch.height, top, widgets);
        let mut list_i = 0usize;
        for (w, &(wy, wh)) in widgets.iter().zip(&layout) {
            match w {
                UiWidget::Button(b) => {
                    let hover = ui::hovered(self.cursor, x, wy, MOD_UI_COLUMN, 20.0);
                    let label = Text::literal(b.label.clone());
                    ui::button(
                        batch,
                        atlas,
                        x,
                        wy,
                        MOD_UI_COLUMN,
                        20.0,
                        &label,
                        true,
                        hover,
                    );
                }
                UiWidget::Text(t) => {
                    let color = if t.color == 0 {
                        u32::MAX
                    } else {
                        t.color | 0xFF00_0000
                    };
                    ui::centered_text(
                        batch,
                        atlas,
                        &Text::literal(t.text.clone()),
                        x,
                        wy + 1.0,
                        MOD_UI_COLUMN,
                        color,
                    );
                }
                UiWidget::Item(i) => {
                    if self.res.items().get(&i.item).is_some() {
                        items.push((i.item.clone(), x + 2.0, wy + 2.0));
                    } else {
                        batch.text(atlas, "?", x + 6.0, wy + 6.0, u32::MAX, true);
                    }
                    let name = if i.name.is_empty() {
                        i.item.rsplit(':').next().unwrap_or(&i.item).to_string()
                    } else {
                        i.name.clone()
                    };
                    batch.text_styled(
                        atlas,
                        &Text::literal(name),
                        x + 22.0,
                        wy + 6.0,
                        u32::MAX,
                        true,
                    );
                }
                UiWidget::List(l) => {
                    ms.scrolls.resize_with(list_i + 1, ui::Scroll::default);
                    let scroll = ms.scrolls[list_i];
                    list_i += 1;
                    let rows_top = wy + 4.0;
                    let rows_bottom = wy + wh - 4.0;
                    batch.fill(atlas, x, wy, MOD_UI_COLUMN, wh, 0x8000_0000);
                    for (b, yy) in [(x, wy), (x, wy + wh - 1.0)] {
                        batch.fill(atlas, b, yy, MOD_UI_COLUMN, 1.0, 0xFF50_5050);
                    }
                    for (xx, yy) in [(x, wy), (x + MOD_UI_COLUMN - 1.0, wy)] {
                        batch.fill(atlas, xx, yy, 1.0, wh, 0xFF50_5050);
                    }
                    let content = l.rows.len() as f32 * 12.0;
                    draw_list_rows(
                        batch,
                        atlas,
                        self.cursor,
                        &l.rows,
                        l.selected,
                        scroll.amount,
                        (x, wy, wh),
                    );
                    ui::scrollbar(
                        batch,
                        atlas,
                        x + MOD_UI_COLUMN - 5.0,
                        rows_top,
                        rows_bottom,
                        content,
                        scroll.amount,
                    );
                }
            }
        }
        let hover = ui::hovered(self.cursor, cx - 100.0, batch.height - 27.0, 200.0, 20.0);
        ui::button(
            batch,
            atlas,
            cx - 100.0,
            batch.height - 27.0,
            200.0,
            20.0,
            &Text::literal("Done"),
            true,
            hover,
        );
        for (id, ix, iy) in items {
            let stack = ItemStack::new(id.as_str(), 1);
            self.draw_item(batch, &stack, ix, iy);
        }
    }
    /// A click on the mod screen: Done leaves it, a `list` row is the mod's
    /// `ui-events::select`.
    pub(super) fn mod_screen_click(&mut self, button: MouseButton) {
        // Key Binds: a mouse button pressed while waiting binds it.
        if self
            .mods
            .screen
            .as_ref()
            .is_some_and(|ms| ms.binding.is_some())
        {
            self.bind_mod_key(keybinds::mouse_name(button));
            return;
        }
        if button != MouseButton::Left {
            return;
        }
        let (w, h) = self.gui_size();
        let cursor = self.cursor;
        if ui::hovered(cursor, (w / 2.0).floor() - 100.0, h - 27.0, 200.0, 20.0) {
            self.audio.play_button_click();
            self.close_wasm_mod_screen(true);
            return;
        }
        let Some(ms) = self.mods.screen.as_ref() else {
            return;
        };
        let Some((_, widgets)) = Self::mod_ui_view(&self.mods.mods, ms) else {
            return;
        };
        let owner = ms.owner;
        let x = (w / 2.0).floor() - MOD_UI_COLUMN / 2.0;
        // A key row waits for the key to bind.
        let keys = Self::mod_page_keys(&self.mods.mods, ms);
        let picked = (0..keys.len()).find(|&i| {
            let wy = MOD_UI_TOP + i as f32 * MOD_UI_KEY_ROW;
            ui::hovered(cursor, x, wy, MOD_UI_COLUMN, 20.0)
        });
        if let Some(i) = picked {
            let name = keys[i].name.clone();
            self.audio.play_button_click();
            if let Some(ms) = self.mods.screen.as_mut() {
                ms.binding = Some(name);
            }
            return;
        }
        let top = MOD_UI_TOP + keys.len() as f32 * MOD_UI_KEY_ROW;
        let layout = Self::mod_ui_layout(h, top, widgets);
        let mut list_i = 0usize;
        // The `ui_select` call needs `&mut`: find the pick first.
        let mut hit = None;
        for (w, &(wy, wh)) in widgets.iter().zip(&layout) {
            // A button is `select(id, 0)`.
            if let UiWidget::Button(b) = w
                && ui::hovered(cursor, x, wy, MOD_UI_COLUMN, 20.0)
            {
                self.audio.play_button_click();
                let owner = self.mods.screen.as_ref().map_or(0, |ms| ms.owner);
                self.mods.mods.ui_select(owner, b.id, 0);
                self.show_mod_output();
                return;
            }
            let UiWidget::List(l) = w else { continue };
            let scroll = ms.scrolls.get(list_i).copied().unwrap_or_default();
            list_i += 1;
            let row = ((cursor.1 - (wy + 4.0) + scroll.amount) / 12.0) as i64;
            if ui::hovered(cursor, x + 1.0, wy + 1.0, MOD_UI_COLUMN - 7.0, wh - 2.0)
                && row >= 0
                && (row as usize) < l.rows.len()
            {
                hit = Some((l.id, row as u32));
                break;
            }
        }
        if let Some((id, row)) = hit {
            self.audio.play_button_click();
            self.mods.mods.ui_select(owner, id, row);
            self.show_mod_output();
        }
    }
    /// The wheel over a `list` scrolls it.
    pub(super) fn mod_screen_scroll(&mut self, delta: f32) {
        let (w, h) = self.gui_size();
        let cursor = self.cursor;
        let Some(ms) = self.mods.screen.as_mut() else {
            return;
        };
        let Some((_, widgets)) = Self::mod_ui_view(&self.mods.mods, ms) else {
            return;
        };
        let x = (w / 2.0).floor() - MOD_UI_COLUMN / 2.0;
        let top =
            MOD_UI_TOP + Self::mod_page_keys(&self.mods.mods, ms).len() as f32 * MOD_UI_KEY_ROW;
        let layout = Self::mod_ui_layout(h, top, widgets);
        let mut list_i = 0usize;
        for (w, &(wy, wh)) in widgets.iter().zip(&layout) {
            let UiWidget::List(l) = w else { continue };
            let over = ui::hovered(cursor, x, wy, MOD_UI_COLUMN, wh);
            ms.scrolls.resize_with(list_i + 1, ui::Scroll::default);
            if over {
                let content = l.rows.len() as f32 * 12.0;
                ms.scrolls[list_i].wheel(delta, 12.0, wy + 4.0, wy + wh - 4.0, content);
                return;
            }
            list_i += 1;
        }
    }
}

/// A mod screen's `list` rows in its frame at (`x`, `wy`), `wh` high, scrolled by `scroll`: the
/// rows in view, the hovered one lit and the selected one marked. As AbstractSelectionList draws
/// its entries inside `enableScissor`, they are cut at the frame's inside: the row a scroll shows
/// in part, and the one below a full view, never draw over the frame or below it.
#[cfg(feature = "mods")]
fn draw_list_rows(
    batch: &mut HudBatch,
    atlas: &horizon_client::hud::HudAtlas,
    cursor: (f32, f32),
    rows: &[String],
    selected: Option<u32>,
    scroll: f32,
    (x, wy, wh): (f32, f32, f32),
) {
    let rows_top = wy + 4.0;
    let rows_bottom = wy + wh - 4.0;
    let first = (scroll / 12.0) as usize;
    let last = (first + ((rows_bottom - rows_top) / 12.0).ceil() as usize + 1).min(rows.len());
    let start = batch.vertices.len();
    for (vi, ri) in (first..last).enumerate() {
        let ry = rows_top + vi as f32 * 12.0 - (scroll % 12.0);
        if ui::hovered(cursor, x, ry, MOD_UI_COLUMN - 6.0, 12.0) {
            batch.fill(atlas, x + 1.0, ry, MOD_UI_COLUMN - 8.0, 12.0, 0x20FF_FFFF);
        }
        if selected == Some(ri as u32) {
            batch.fill(atlas, x + 1.0, ry, 3.0, 12.0, 0xFFFF_FFFF);
        }
        batch.text_styled(
            atlas,
            &Text::literal(rows[ri].clone()),
            x + 6.0,
            ry + 2.0,
            u32::MAX,
            true,
        );
    }
    batch.clip_since(
        start,
        x + 1.0,
        wy + 1.0,
        x + MOD_UI_COLUMN - 1.0,
        wy + wh - 1.0,
    );
}

#[cfg(feature = "mods")]
const MOD_UI_TOP: f32 = 40.0;
#[cfg(feature = "mods")]
const MOD_UI_COLUMN: f32 = 200.0;
/// A key row's height with its gap, as the widgets below it space out.
#[cfg(feature = "mods")]
const MOD_UI_KEY_ROW: f32 = 26.0;

#[cfg(all(test, feature = "mods"))]
mod tests {
    use super::*;
    use horizon_client::hud::HudAtlas;

    #[test]
    fn list_rows_stay_inside_the_frame_scrolled_or_not() {
        let atlas =
            HudAtlas::build(std::path::Path::new("/nonexistent/horizon-test-assets")).unwrap();
        // #3237: 22 rows in a frame of 6 (mod_ui_layout's height for a long list). The 7th row
        // (index 6), drawn for a scroll that shows it in part, is selected and under the
        // cursor, so its fills reach past the frame unless they are cut.
        let rows: Vec<String> = (0..22).map(|i| format!("Row {i}")).collect();
        let (x, wy, wh) = (100.0, 40.0, 6.0 * 12.0 + 8.0);
        for scroll in [0.0, 6.0, 30.0] {
            let mut batch = HudBatch::new([640, 480]);
            let seventh_row = wy + 4.0 + 6.0 * 12.0 - scroll % 12.0;
            draw_list_rows(
                &mut batch,
                &atlas,
                (x + 10.0, seventh_row + 2.0),
                &rows,
                Some(6 + (scroll / 12.0) as u32),
                scroll,
                (x, wy, wh),
            );
            assert!(!batch.vertices.is_empty(), "scroll {scroll}: rows drawn");
            let (top, bottom) = batch
                .vertices
                .iter()
                .map(|v| v.pos[1])
                .fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
            assert!(
                top >= wy + 1.0 && bottom <= wy + wh - 1.0,
                "scroll {scroll}: rows span {top}..{bottom}, the frame's inside is {}..{}",
                wy + 1.0,
                wy + wh - 1.0
            );
        }
    }
}

/// A mod gave `block` block entity `id`, but the block's template has none,
/// so breaking it would never remove it: the block gets none.
#[allow(dead_code)]
pub(super) fn warn_no_block_entity(block: &str, id: &str) {
    log::warn!("{block}: no block entity {id}, its template has none (copy a chest or furnace)");
}

/// A WebAssembly code mod's row on the Mods page.
const STOPPED: &str = "Doesn't run any more: needs a native port";
/// The player was told once about the WebAssembly mods that stopped (`horizon-mods/<this>`).
const NOTICE_SEEN: &str = ".native-notice";

impl Game<'_> {
    /// Lists the installed mods on the Mods page. Content mods load with the resources
    /// (`content_root`); native ones are built into the game (`native_hooks.rs`). WebAssembly code
    /// mods don't run any more: their rows say so, and the player hears it once in the chat
    /// (`tell_native_now`).
    pub(super) fn start_mods(&mut self) {
        let dir = Self::game_dir().join(mod_list::DIR);
        let mut paths = horizon_mods::find(&dir);
        paths.retain(|p| !self.mods.dev.contains(p));
        paths.extend(self.mods.dev.iter().cloned());
        let (mut content_ids, mut live_ids) = Default::default();
        self.settings.mods.installed = paths
            .iter()
            .map(|path| {
                let file = path.file_name().unwrap_or_default().to_string_lossy();
                let package = match horizon_mods::Package::read(path) {
                    Ok(package) => package,
                    Err(e) => {
                        return Installed {
                            id: String::new(),
                            title: file.into_owned(),
                            description: String::new(),
                            problem: Some(format!("Can't be read: {e}")),
                            dev: false,
                            has_settings: false,
                            native: None,
                        };
                    }
                };
                let m = &package.manifest;
                let problem = package.wasm.is_some().then(|| STOPPED.to_owned());
                let dev = self.mods.dev.contains(path);
                live_switch_of(path, &package, dev, &mut content_ids, &mut live_ids);
                Installed {
                    id: m.id.clone(),
                    title: format!("{} {}", m.name, m.version),
                    description: m.description.clone(),
                    problem,
                    dev,
                    has_settings: false,
                    native: None,
                }
            })
            .collect();
        (self.mods.content_ids, self.mods.live_ids) = (content_ids, live_ids);
        self.ui.options.mods_live = self.mods_live_now();
        if self.mods.applied.is_none() {
            self.mods.applied = Some(self.settings.mods.enabled.clone());
        }
        // A mod with a native version built in says so instead, and isn't news.
        self.list_native_mods();
        self.sync_native_switches();
        let built = std::env::current_exe()
            .ok()
            .and_then(|exe| std::fs::read_to_string(exe.with_file_name(BUILD_INFO)).ok())
            .unwrap_or_default();
        let built_mods = native_mods_detail(&built);
        let built_with_mods = built_mods.is_some();
        if let Some(list) = built_mods {
            horizon_client::crash::set_detail("Native Mods", list);
        }
        horizon_client::gfx::native_render::set_mods_built_in(
            built_with_mods
                || built_with_native_mods()
                || self
                    .settings
                    .mods
                    .installed
                    .iter()
                    .any(|m| m.native.is_some()),
        );
        let native_dir = Self::game_dir().join(mod_list::NATIVE_DIR);
        let unbuilt = not_built(&native_dir, &built, &self.settings.mods.installed);
        self.settings.mods.installed.extend(unbuilt);
        let installed = &self.settings.mods.installed;
        self.mods.stopped = installed
            .iter()
            .filter(|row| row.problem.as_deref() == Some(STOPPED))
            .map(|row| row.title.clone())
            .collect();
    }
    /// The content mods the Mods page lets the player switch with a world
    /// open: in single player and on an open LAN world, those with only
    /// assets and data; none on another server, which owns its data.
    pub(super) fn mods_live_now(&self) -> std::collections::BTreeSet<String> {
        match self.integrated.is_some() || self.remote.is_none() {
            true => self.mods.live_ids.clone(),
            false => Default::default(),
        }
    }
    /// The content mods the resources load with: the ones applied, which a
    /// switch that needs a restart doesn't change.
    pub(super) fn applied_mods(&self) -> std::collections::BTreeSet<String> {
        self.mods
            .applied
            .clone()
            .unwrap_or_else(|| self.settings.mods.enabled.clone())
    }
    /// The player turned content mods on or off (Options > Mods, or an
    /// install from Browse Mods): those with only assets and data apply now
    /// (docs/design/mod-hotload.md), through the resource reload and then the
    /// server's data reload (`mods_reloaded`); those that add blocks, items or
    /// worldgen wait for a restart, and the Mods page says so.
    pub(super) fn switch_mods(&mut self) {
        let Some(applied) = self.mods.applied.clone() else {
            return;
        };
        let wanted = self.settings.mods.enabled.clone();
        if applied == wanted {
            return;
        }
        // Native mods' `mod.<id>` lines follow the player's choice as they are
        // (`sync_native_switches`); a content mod with blocks, items or
        // worldgen keeps the state it loaded with until a restart.
        let content = &self.mods.content_ids;
        let mut next = wanted.clone();
        let mut waiting = Vec::new();
        for id in applied.symmetric_difference(&wanted) {
            if !content.contains(id) || self.mods.live_ids.contains(id) {
                continue;
            }
            log::info!("Horizon: mod {id} needs a restart to switch");
            waiting.push(id.clone());
            if applied.contains(id) {
                next.insert(id.clone());
            } else {
                next.remove(id);
            }
        }
        self.ui.options.mods_note = (!waiting.is_empty())
            .then(|| lang::tr("horizon.mods.switch.restart", &[&waiting.join(", ")]));
        let changed = applied
            .symmetric_difference(&next)
            .any(|id| content.contains(id));
        if !changed {
            self.mods.applied = Some(next);
            return;
        }
        log::info!(
            "Horizon: content mods switched live: {:?} -> {:?}",
            applied,
            next
        );
        self.mods.applied = Some(next);
        self.mods.live = Some(std::time::Instant::now());
        self.reload_packs();
    }
    /// A live switch's resources applied: the server reloads its data packs
    /// from the new root, as `/reload` does (recipes, tags, loot tables,
    /// advancements, functions), and sends them to its players.
    pub(super) fn mods_reloaded(&mut self, applied: bool) {
        let Some(started) = self.mods.live.take() else {
            return;
        };
        let resources_ms = started.elapsed().as_millis();
        if !applied {
            return;
        }
        let root = self.options.resources.clone();
        let enabled = self.applied_mods();
        let reload = move |g: &mut Game<'_>| {
            let at = std::time::Instant::now();
            g.options.resources = root;
            g.settings.mods.enabled = enabled.clone();
            g.mods.applied = Some(enabled);
            g.mods.data_switched = true;
            if g.world_root.as_os_str().is_empty() {
                return;
            }
            g.reload_datapacks(true);
            log::info!(
                "Horizon: content mods switched live: server data reloaded in {} ms",
                at.elapsed().as_millis()
            );
        };
        log::info!("Horizon: content mods switched live: resources reloaded in {resources_ms} ms");
        if let Some(server) = &self.integrated {
            server.post(reload);
        } else if self.remote.is_none() {
            reload(self);
        }
    }
    /// Trait mods follow the player's switches at once
    /// (`mod_api::set_mod_on`): the next dispatch skips a mod turned off.
    /// Mixin code checks `native_on` itself.
    pub(super) fn sync_native_switches(&self) {
        for id in horizon_client::mod_api::mod_ids() {
            let on = !self.settings.mods.disabled.contains(*id);
            horizon_client::mod_api::set_mod_on(id, on);
        }
    }
    /// Once, in the first world after the update: which WebAssembly mods stopped.
    pub(super) fn tell_native_now(&mut self) {
        if self.mods.stopped.is_empty() || self.player.is_none() {
            return;
        }
        let names = std::mem::take(&mut self.mods.stopped).join(", ");
        let seen = Self::game_dir().join(mod_list::DIR).join(NOTICE_SEEN);
        if seen.exists() {
            return;
        }
        self.system_message(Text::literal(format!(
            "Horizon mods are native now: {names} stopped running until they have a native version. The launcher's Mods page has the native ones."
        )));
        let _ = std::fs::write(&seen, "");
    }
}

/// Mob types loaded from a scratch root whose `entity-types.json` has a
/// zombie, a villager, and the rows a content root writes for a mod's types
/// (`mod_content`): `ruby:ghoul` (a zombie with 40 health), `ruby:settler` (a
/// villager as it is) and `ruby:bolt` (a thrown snowball, not a mob).
#[cfg(test)]
pub(super) fn living_mod_types()
-> std::collections::HashMap<String, std::sync::Arc<horizon_client::mobs::MobType>> {
    let zombie =
        r#""width":0.6,"height":1.95,"eye_height":1.74,"category":"monster","fire_immune":false"#;
    let villager =
        r#""width":0.6,"height":1.95,"eye_height":1.62,"category":"misc","fire_immune":false"#;
    let trader =
        r#""width":0.6,"height":1.95,"eye_height":1.62,"category":"creature","fire_immune":false"#;
    let table = format!(
        r#"{{"minecraft:zombie":{{{zombie},"attributes":{{"minecraft:max_health":20.0,"minecraft:movement_speed":0.23000000417232513}}}},
        "minecraft:villager":{{{villager},"attributes":{{"minecraft:max_health":20.0,"minecraft:movement_speed":0.5}}}},
        "minecraft:wandering_trader":{{{trader},"attributes":{{"minecraft:max_health":20.0,"minecraft:movement_speed":0.7}}}},
        "minecraft:snowball":{{"width":0.25,"height":0.25,"eye_height":0.0,"category":"misc","fire_immune":false}},
        "ruby:ghoul":{{{zombie},"attributes":{{"minecraft:max_health":40.0,"minecraft:movement_speed":0.23000000417232513}},"template":"minecraft:zombie"}},
        "ruby:settler":{{{villager},"attributes":{{"minecraft:max_health":20.0,"minecraft:movement_speed":0.5}},"template":"minecraft:villager"}},
        "ruby:merchant":{{{trader},"attributes":{{"minecraft:max_health":20.0,"minecraft:movement_speed":0.7}},"template":"minecraft:wandering_trader"}},
        "ruby:bolt":{{"width":0.25,"height":0.25,"eye_height":0.0,"category":"misc","fire_immune":false,"template":"minecraft:snowball"}}}}"#
    );
    // A folder per call: tests run in parallel in one process, and each removes its own.
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "horizon-living-mod-types-{}-{call}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("entity-types.json"), table).unwrap();
    let types = horizon_client::mobs::MobTypes::load_for_server(&root).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    types
        .types
        .into_iter()
        .map(|(id, t)| (id, std::sync::Arc::new(t)))
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn sound_mod_dispatch_changes_real_channels_and_cancellation_short_circuits() {
        use crate::mod_api::{Flow, Hooks, SoundPlayEvent};
        use horizon_client::sound_engine::{
            Attenuation, PlayResult, SoundEngine, SoundInstance, test_backend,
        };
        use horizon_client::sound_events::SoundRegistry;
        use horizon_core::random::LegacyRandom;
        struct First;
        impl Hooks for First {
            fn sound_play(&self, event: &mut SoundPlayEvent<'_>) -> Flow {
                assert_eq!(event.sound.identifier, "test:beep");
                event.volume *= 0.5;
                Flow::Continue
            }
        }
        struct Second;
        impl Hooks for Second {
            fn sound_play(&self, event: &mut SoundPlayEvent<'_>) -> Flow {
                assert_eq!(event.volume, 0.4);
                event.pitch = 2.0;
                Flow::Continue
            }
        }
        struct Cancel;
        impl Hooks for Cancel {
            fn sound_play(&self, _: &mut SoundPlayEvent<'_>) -> Flow {
                Flow::Cancel
            }
        }
        struct Never;
        impl Hooks for Never {
            fn sound_play(&self, _: &mut SoundPlayEvent<'_>) -> Flow {
                panic!("dispatch after cancellation")
            }
        }
        static MODS: std::sync::LazyLock<Vec<Box<dyn Hooks>>> =
            std::sync::LazyLock::new(|| vec![Box::new(First), Box::new(Second)]);
        static CANCEL: std::sync::LazyLock<Vec<Box<dyn Hooks>>> =
            std::sync::LazyLock::new(|| vec![Box::new(Cancel), Box::new(Never)]);
        let mut registry = SoundRegistry::default();
        registry.add_file("test", br#"{"beep":{"sounds":["beep"]}}"#, &|_, _| true);
        let mut engine = SoundEngine::new(registry);
        engine.listening = true;
        let mut backend = test_backend::Null::default();
        let instance = || {
            SoundInstance::new(
                "test:beep",
                4,
                0.8,
                1.0,
                LegacyRandom::new(1),
                false,
                0,
                Attenuation::None,
                [0.0; 3],
                true,
            )
        };
        engine.set_test_playback_hook(|sound, volume, pitch, cancel| {
            crate::mod_api::dispatch_sound_play(&MODS, sound, volume, pitch, cancel)
        });
        let (id, result) = engine.play_new(instance(), false, &mut backend);
        assert_eq!(result, PlayResult::Started);
        let channel = engine.channel(id).unwrap();
        assert_eq!(backend.volume[&channel], 0.4);
        assert_eq!(backend.pitch[&channel], 2.0);
        let notifications = engine.notifications.len();
        engine.set_test_playback_hook(|sound, volume, pitch, cancel| {
            crate::mod_api::dispatch_sound_play(&CANCEL, sound, volume, pitch, cancel)
        });
        let (_, result) = engine.play_new(instance(), false, &mut backend);
        assert_eq!(result, PlayResult::NotStarted);
        assert_eq!(backend.volume.len(), 1);
        assert_eq!(engine.notifications.len(), notifications);
    }
    use super::*;
    use horizon_client::mod_list::Installed;
    use std::io::Write;

    #[test]
    #[ignore = "prepared resources: run explicitly for native content"]
    fn built_native_sound_content_reaches_the_resource_root_and_obeys_disable() {
        let base = std::path::PathBuf::from(
            std::env::var_os("HORIZON_TEST_RESOURCES").expect("prepared resources required"),
        );
        let scratch =
            std::env::temp_dir().join(format!("horizon-native-content-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        let version = scratch.join("version");
        let home = scratch.join("game");
        std::fs::create_dir_all(&home).unwrap();
        let content = version.join("native-content/soundproof");
        std::fs::create_dir_all(content.join("assets/soundproof/sounds")).unwrap();
        std::fs::write(content.join("horizon-mod.toml"), "id=\"soundproof\"\nname=\"Sound proof\"\nversion=\"1\"\napi=\"0.2\"\ncapabilities=[\"registries\"]\n").unwrap();
        std::fs::write(
            content.join("assets/soundproof/sounds.json"),
            r#"{"proof":{"sounds":["soundproof:proof"]}}"#,
        )
        .unwrap();
        std::fs::write(
            content.join("assets/soundproof/sounds/proof.ogg"),
            b"synthetic existence marker; no audio decode",
        )
        .unwrap();
        let legacy = home.join(mod_list::DIR).join("soundproof");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::copy(
            content.join("horizon-mod.toml"),
            legacy.join("horizon-mod.toml"),
        )
        .unwrap();
        let enabled = ["soundproof".to_owned()].into();
        let empty = Default::default();
        let root = content_root_at(&base, &empty, &empty, &[], &home, Some(&version));
        assert_ne!(root, base);
        let sounds = horizon_client::sound_events::SoundRegistry::load(&root);
        assert!(!sounds.tracks("soundproof:proof").is_empty());
        let disabled = ["soundproof".to_owned()].into();
        let off = content_root_at(&base, &enabled, &disabled, &[], &home, Some(&version));
        assert_eq!(off, base, "disabled native mods contribute no assets");
        std::fs::remove_dir_all(scratch).unwrap();
    }

    /// A content mod's living type is a mob of its template's species under
    /// its own id, with its own attributes: it saves its id with every field
    /// its template's class saves, loads back as itself, and rolls its own
    /// loot table. A thrown type stays a projectile.
    #[test]
    fn a_mod_living_type_is_its_templates_mob_with_its_own_attributes() {
        use horizon_client::mobs::{Mob, Species, drops::death_loot_table};
        use horizon_core::geometry::Vec3;
        let types = living_mod_types();
        let (zombie, ghoul) = (&types["minecraft:zombie"], &types["ruby:ghoul"]);
        assert_eq!(ghoul.species, Species::Zombie);
        assert_eq!(ghoul.class_id(), "minecraft:zombie");
        assert_eq!(types["ruby:settler"].species, Species::Villager);
        assert!(!types.contains_key("ruby:bolt"), "a thrown type is no mob");
        assert_eq!((zombie.max_health, ghoul.max_health), (20.0, 40.0));
        assert_eq!(ghoul.speed.to_bits(), zombie.speed.to_bits());
        assert_eq!(
            (ghoul.sounds, ghoul.texture.as_str()),
            (zombie.sounds, zombie.texture.as_str())
        );

        let at = Vec3::new(0.5, 64.0, 0.5);
        let (zombie, ghoul) = (
            Mob::new(1, zombie.clone(), at, 0.0),
            Mob::new(2, ghoul.clone(), at, 0.0),
        );
        assert_eq!(ghoul.health, 40.0);
        let saved =
            |m: &Mob| horizon_client::entity_nbt::mob_to_nbt(m, [0, 0, 0, 2], None, None, |_| None);
        let keys = |tag: &horizon_world::nbt::Tag| -> Vec<String> {
            let horizon_world::nbt::Tag::Compound(map) = tag else {
                panic!("a mob saves a compound");
            };
            map.keys().map(|k| k.to_string_lossy()).collect()
        };
        let (zombie_tag, ghoul_tag) = (saved(&zombie), saved(&ghoul));
        let Some(horizon_world::nbt::Tag::String(id)) = ghoul_tag.get("id") else {
            panic!("a saved mob has its id");
        };
        assert_eq!(id.to_string_lossy(), "ruby:ghoul");
        assert_eq!(
            keys(&ghoul_tag),
            keys(&zombie_tag),
            "it saves what a zombie saves"
        );
        for key in ["IsBaby", "CanBreakDoors", "DrownedConversionTime"] {
            assert!(keys(&ghoul_tag).iter().any(|k| k == key), "{key}");
        }
        let loaded = horizon_client::entity_nbt::mob_from_nbt(&ghoul_tag, &types, 3, None)
            .expect("it loads back");
        assert_eq!(
            (loaded.kind.id.as_str(), loaded.health),
            ("ruby:ghoul", 40.0)
        );
        assert_eq!(
            death_loot_table(&ghoul, true).as_deref(),
            Some("ruby:entities/ghoul")
        );
        assert_eq!(
            death_loot_table(&zombie, true).as_deref(),
            Some("minecraft:entities/zombie")
        );
    }

    /// A content mod's type from the wandering trader, the neutral template
    /// whose behaviour is all goals: a trader under its own id, with a
    /// trader's goals, that saves what a trader saves and loads back.
    #[test]
    fn a_mod_trader_type_is_a_wandering_trader_under_its_own_id() {
        use horizon_client::mobs::{Mob, Species};
        use horizon_world::nbt::Tag;
        let types = living_mod_types();
        let merchant = &types["ruby:merchant"];
        assert_eq!(merchant.species, Species::WanderingTrader);
        assert_eq!(merchant.class_id(), "minecraft:wandering_trader");
        let at = horizon_core::geometry::Vec3::new(0.5, 64.0, 0.5);
        let trader = Mob::new(1, types["minecraft:wandering_trader"].clone(), at, 0.0);
        let merchant = Mob::new(2, merchant.clone(), at, 0.0);
        assert!(merchant.ai.goals.has("Float") && trader.ai.goals.has("Float"));
        let saved =
            |m: &Mob| horizon_client::entity_nbt::mob_to_nbt(m, [0, 0, 0, 2], None, None, |_| None);
        let keys = |tag: &Tag| -> Vec<String> {
            let Tag::Compound(map) = tag else {
                panic!("a mob saves a compound");
            };
            map.keys().map(|k| k.to_string_lossy()).collect()
        };
        let tag = saved(&merchant);
        assert_eq!(
            keys(&tag),
            keys(&saved(&trader)),
            "it saves what a trader saves"
        );
        let loaded =
            horizon_client::entity_nbt::mob_from_nbt(&tag, &types, 3, None).expect("it loads back");
        assert_eq!(
            (loaded.kind.id.as_str(), loaded.species()),
            ("ruby:merchant", Species::WanderingTrader)
        );
    }

    /// A mod's live-switch verdict is read once per version of the mod: a later look finds
    /// it without reading the files again, a `--dev-mod` is always read, and the mod
    /// replaced (a new modification time) is read again.
    #[test]
    fn a_mods_live_switch_verdict_is_read_once_per_version_of_it() {
        let dir = std::env::temp_dir().join(format!("horizon-verdict-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ruby = dir.join("ruby");
        std::fs::create_dir_all(ruby.join("assets/ruby/lang")).unwrap();
        std::fs::create_dir_all(ruby.join("data/ruby")).unwrap();
        std::fs::write(ruby.join("assets/ruby/lang/en_us.json"), "{}").unwrap();
        let manifest = horizon_mods::Manifest::parse(
            "id = \"ruby\"\nname = \"Ruby\"\nversion = \"0.1.0\"\napi = \"0.2\"\n\
             side = \"both\"\ncapabilities = [\"registries\"]\n",
        )
        .unwrap();
        assert_eq!(verdict(&ruby, &manifest, false), Some(Ok(())));
        // Worldgen added below `data/ruby`: the mod folder's own time stays.
        let worldgen = ruby.join("data/ruby/worldgen/configured_feature");
        std::fs::create_dir_all(&worldgen).unwrap();
        std::fs::write(worldgen.join("ore.json"), "{}").unwrap();
        let switches_on_restart = Some(Err("horizon.mods.switch.worldgen"));
        assert_eq!(verdict(&ruby, &manifest, true), switches_on_restart);
        assert_eq!(verdict(&ruby, &manifest, false), Some(Ok(())));
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        let mut folder = std::fs::OpenOptions::new();
        folder.read(true);
        // Windows opens a folder only with FILE_FLAG_BACKUP_SEMANTICS, and sets its time only
        // with write access.
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            folder.write(true).custom_flags(0x0200_0000);
        }
        folder.open(&ruby).unwrap().set_modified(later).unwrap();
        assert_eq!(verdict(&ruby, &manifest, false), switches_on_restart);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The mods read on threads at once come back in the order given, each with its own
    /// files, and a refused mod's error stays with that mod alone.
    #[test]
    fn mods_read_at_once_keep_their_order_and_their_own_errors() {
        let dir = std::env::temp_dir().join(format!("horizon-read-at-once-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths: Vec<PathBuf> = (0..12).map(|i| dir.join(format!("m{i}"))).collect();
        for (i, path) in paths.iter().enumerate() {
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(
                path.join("horizon-mod.toml"),
                format!(
                    "id = \"m{i}\"\nname = \"M\"\nversion = \"0.1.0\"\napi = \"0.2\"\n\
                     side = \"both\"\ncapabilities = [\"registries\"]\n"
                ),
            )
            .unwrap();
            // Mod i has i + 1 language files; mod 5 also writes into another mod's namespace.
            let lang = path.join(format!("assets/m{i}/lang"));
            std::fs::create_dir_all(&lang).unwrap();
            for n in 0..=i {
                std::fs::write(lang.join(format!("l{n}.json")), "{}").unwrap();
            }
            if i == 5 {
                std::fs::create_dir_all(path.join("assets/m0/lang")).unwrap();
                std::fs::write(path.join("assets/m0/lang/x.json"), "{}").unwrap();
            }
        }
        let packages: Vec<(&PathBuf, horizon_mods::Package)> = paths
            .iter()
            .map(|p| (p, horizon_mods::Package::read(p).unwrap()))
            .collect();
        let read = read_contents(&packages);
        assert_eq!(read.len(), 12);
        for (i, content) in read.iter().enumerate() {
            if i == 5 {
                assert!(content.as_ref().unwrap_err().contains("own namespaces"));
                continue;
            }
            let files = &content.as_ref().unwrap().files;
            assert_eq!(files.len(), i + 1, "mod {i}");
            assert!(
                files
                    .iter()
                    .all(|f| f.starts_with(&format!("assets/m{i}/")))
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Only a build whose `native-mods.json` lists a mod counts as built with mods.
    #[test]
    fn only_a_listed_mod_makes_a_build_a_mod_build() {
        assert!(!build_lists_mods(""));
        assert!(!build_lists_mods("{}"));
        assert!(!build_lists_mods(r#"{"mods":[]}"#));
        assert!(!build_lists_mods(r#"{"mods":"worldmap"}"#));
        assert!(build_lists_mods(r#"{"mods":[{"id":"worldmap"}]}"#));
    }

    /// A minimal `.hnmod` the registry accepts: its manifest and its mixin's.
    fn hnmod(dir: &std::path::Path, id: &str) {
        let horizon = "a".repeat(40);
        let manifest = serde_json::json!({
            "schema": 1, "id": id, "version": "0.3.0", "game": "26.3",
            "horizon": horizon, "mixins": "b".repeat(40),
        });
        let mixin = format!(
            "[mod]\nid = \"{id}\"\nversion = \"0.3.0\"\n[compatibility]\nhorizon_revision = \"{horizon}\"\n"
        );
        let file = std::fs::File::create(dir.join(format!("{id}.hnmod"))).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("horizon-native.json", options).unwrap();
        zip.write_all(manifest.to_string().as_bytes()).unwrap();
        zip.start_file(format!("mixins/{id}/horizon-mixin.toml"), options)
            .unwrap();
        zip.write_all(mixin.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn the_crash_report_lists_the_builds_native_mods() {
        let built = r#"{"mods":[{"id":"machine_sample","version":"0.1.0","sha256":null},{"id":"pipes","version":"2.0"}]}"#;
        assert_eq!(
            native_mods_detail(built).as_deref(),
            Some("machine_sample 0.1.0, pipes 2.0")
        );
        // A release has no file; a build without mods has an empty list.
        assert_eq!(native_mods_detail(""), None);
        assert_eq!(native_mods_detail(r#"{"mods":[]}"#), None);
    }

    #[test]
    fn native_mods_installed_but_not_built_get_a_row_saying_so() {
        let dir = std::env::temp_dir().join(format!("horizon-not-built-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for id in ["veinminer", "zoom", "minimap"] {
            hnmod(&dir, id);
        }
        std::fs::write(dir.join("notes.txt"), "not a mod").unwrap();
        std::fs::write(dir.join("broken.hnmod"), "not a zip").unwrap();
        // A release: none is built in; the page lists them, sorted, instead of "No mods".
        let rows = not_built(&dir, "", &[]);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["minimap", "veinminer", "zoom"]);
        assert_eq!(rows[1].title, "veinminer 0.3.0");
        assert_eq!(rows[1].problem.as_deref(), Some(mod_list::NOT_BUILT));
        assert!(rows.iter().all(|r| r.native.is_none() && !r.dev));
        // A native build with zoom, and minimap's row already listed by its own hook.
        let built =
            r#"{"base":"nightly-2026-10-30-1da354b","mods":[{"id":"zoom","version":"0.3.0"}]}"#;
        let listed = Installed {
            id: "minimap".into(),
            ..rows[0].clone()
        };
        let rows = not_built(&dir, built, &[listed]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "veinminer");
        // No folder: no rows.
        assert!(not_built(&dir.join("missing"), "", &[]).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A trait mod's `register_goals` (`Mob.registerGoals`) gets every new
    /// mob: the goals of its type replace its template's, its `Float` goal
    /// floats the navigation as FloatGoal's constructor does, and the
    /// template's other types keep their goals. The only test here that
    /// installs the client library's hooks.
    #[test]
    fn a_mods_register_goals_replaces_its_types_goals() {
        use horizon_client::mobs::{
            Mob,
            goals::{self, Cx, Goal},
        };
        use horizon_client::mod_api::{Hooks, RegisterGoalsEvent, dispatch_register_goals};
        use horizon_client::native_client_hooks::{NativeClientHooks, install};
        use horizon_core::geometry::Vec3;
        // docs/native-mods.md's example.
        #[derive(Clone, Debug)]
        struct ToPost {
            post: [f64; 3],
        }
        impl Goal for ToPost {
            fn flags(&self) -> u8 {
                goals::MOVE
            }
            fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
                let [x, y, z] = self.post;
                let (dx, dy, dz) = (m.position.x - x, m.position.y - y, m.position.z - z);
                dx * dx + dy * dy + dz * dz > 16.0
            }
            fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
                !m.ai.nav.is_done()
            }
            fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
                m.nav_move_to(cx.w, self.post, 1.0);
            }
            fn box_clone(&self) -> Box<dyn Goal> {
                Box::new(self.clone())
            }
        }
        struct Guards;
        impl Hooks for Guards {
            fn register_goals(&self, event: &mut RegisterGoalsEvent<'_>) {
                if event.mob.kind.id == "ruby:guard" {
                    let ai = &mut event.mob.ai;
                    ai.goals.clear();
                    ai.targets.clear();
                    ai.goals.add(0, goals::Float);
                    ai.goals.add(
                        1,
                        ToPost {
                            post: [0.5, 64.0, 0.5],
                        },
                    );
                }
            }
        }
        static MODS: std::sync::LazyLock<Vec<Box<dyn Hooks>>> =
            std::sync::LazyLock::new(|| vec![Box::new(Guards)]);
        let installed = install(NativeClientHooks {
            menu_slot_rules: |_, _, _, _, _| {},
            sound_play: |_, _, _, _| {},
            living_heal: |_, _, _| {},
            player_chat: |_, _, _, _, _| {},
            player_chat_v2: |_, _, _, _, _, _| {},
            chat_decorate: |_, _, _, _| {},
            living_breathe: |_, _, _, _, _| {},
            living_drown: |_, _, _, _, _, _| {},
            living_fall: |_, _, _, _, _| {},
            living_jump: |_, _| {},
            register_goals: |mob| dispatch_register_goals(&MODS, mob),
            player_pre_login: |_, _, _, _| {},
            player_profile: |_, _, _| {},
            ingredient: |_, _, _| {},
            ingredient_test: |_, _, _, _| {},
            log_line: |_, _, _, _| {},
            living_death_message: |_, _, _| {},
            handshake: |_, _, _, _, _| {},
        });
        assert!(installed.is_ok());

        let types = living_mod_types();
        let mut guard = (*types["ruby:ghoul"]).clone();
        guard.id = "ruby:guard".into();
        let at = Vec3::new(0.5, 64.0, 0.5);
        let guard = Mob::new(1, std::sync::Arc::new(guard), at, 0.0);
        let ghoul = Mob::new(2, types["ruby:ghoul"].clone(), at, 0.0);
        assert!(guard.ai.goals.has("Float") && guard.ai.goals.has("ToPost"));
        assert!(!guard.ai.goals.has("RandomLookAround"));
        assert!(!guard.ai.targets.has("NearestTarget"));
        assert!(guard.ai.nav.can_float);
        assert!(ghoul.ai.goals.has("RandomLookAround") && !ghoul.ai.goals.has("ToPost"));
        assert!(ghoul.ai.targets.has("NearestTarget"));
        assert!(!ghoul.ai.nav.can_float, "a zombie has no FloatGoal");
    }
}
