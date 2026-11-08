//! Local Minecraft 26.3 installation driven by Android's system pickers.
use horizon_client::official_install::{self, InstallProgress, InstallStage};
use jni::{
    JNIEnv,
    objects::{JObject, JString},
};
use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Folder,
    Archive,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Archive => "archive",
        }
    }
}

struct Request {
    source: Source,
    home: PathBuf,
}

#[derive(Default)]
struct State {
    request: Option<Request>,
    working: bool,
    progress: InstallProgress,
    detail: Option<String>,
    result: Option<std::result::Result<String, String>>,
}

static STATE: Mutex<State> = Mutex::new(State {
    request: None,
    working: false,
    progress: InstallProgress {
        stage: InstallStage::Metadata,
        done: 0,
        total: 0,
    },
    detail: None,
    result: None,
});

pub fn busy() -> bool {
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.request.is_some() || state.working
}

pub fn progress() -> InstallProgress {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).progress
}

pub fn detail() -> Option<String> {
    STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .detail
        .clone()
}

pub fn take_result() -> Option<std::result::Result<String, String>> {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let result = state.result.take();
    if result.is_some() {
        state.progress = InstallProgress {
            stage: InstallStage::Metadata,
            done: 0,
            total: 0,
        };
        state.detail = None;
    }
    result
}

pub fn begin(source: Source, home: PathBuf) -> std::result::Result<(), String> {
    if busy() || crate::documents::busy() {
        return Err("Finish the current import first".into());
    }
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.request = Some(Request { source, home });
        state.progress = InstallProgress {
            stage: InstallStage::Metadata,
            done: 0,
            total: 0,
        };
        state.detail = Some(format!("Choose a Minecraft {} to import", source.label()));
        state.result = None;
    }
    crate::platform::hide();
    let picked = match source {
        Source::Folder => crate::native_ime::pick_minecraft_folder(),
        Source::Archive => crate::native_ime::pick_minecraft_archive(),
    };
    if !picked {
        picker_failed("No Android document picker is available");
        return Err("No Android document picker is available".into());
    }
    Ok(())
}

pub fn picker_failed(error: &str) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.request = None;
    state.working = false;
    state.detail = None;
    state.result = Some(Err(error.into()));
}

pub fn folder_result(mut env: JNIEnv<'_>, activity: JObject<'_>, result: i32, uri: JString<'_>) {
    handle(Source::Folder, &mut env, activity, result, uri);
}

pub fn archive_result(mut env: JNIEnv<'_>, activity: JObject<'_>, result: i32, uri: JString<'_>) {
    handle(Source::Archive, &mut env, activity, result, uri);
}

fn handle(
    source: Source,
    env: &mut JNIEnv<'_>,
    activity: JObject<'_>,
    result: i32,
    uri: JString<'_>,
) {
    let request = STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .request
        .take();
    let Some(request) = request else {
        return;
    };
    if request.source != source {
        finish(Err("Minecraft import was interrupted".into()));
        return;
    }
    if result != -1 || uri.is_null() {
        finish(Err("Import cancelled".into()));
        return;
    }
    let setup = (|| -> jni::errors::Result<_> {
        let text: String = env.get_string(&uri)?.into();
        let activity = env.new_global_ref(activity)?;
        Ok((env.get_java_vm()?, activity, text))
    })();
    let (vm, activity, uri) = match setup {
        Ok(setup) => setup,
        Err(error) => {
            let _ = env.exception_clear();
            finish(Err(format!("Could not read the selected file: {error}")));
            return;
        }
    };
    if !uri.starts_with("content://") || uri.len() > 8192 {
        finish(Err("Unsupported document URI".into()));
        return;
    }
    STATE.lock().unwrap_or_else(|e| e.into_inner()).working = true;
    let spawned = std::thread::Builder::new()
        .name("Minecraft import".into())
        .spawn(move || {
            let result = (|| -> std::result::Result<String, String> {
                let mut env = vm
                    .attach_current_thread()
                    .map_err(|error| format!("Could not attach the import thread: {error}"))?;
                match source {
                    Source::Folder => {
                        let temporary = temporary_directory()?;
                        let result = (|| -> std::result::Result<String, String> {
                            {
                                let mut copied = |bytes: u64| {
                                    set_detail(format!(
                                        "Copying selected folder - {:.1} MiB",
                                        bytes as f64 / 1048576.0
                                    ))
                                };
                                crate::documents_tree::copy(
                                    &mut env,
                                    activity.as_obj(),
                                    &uri,
                                    &temporary,
                                    &mut copied,
                                )
                                .map_err(|error| error.to_string())?;
                            }
                            set_detail("Checking Minecraft 26.3 files...".into());
                            official_install::install_from_directory(
                                &request.home,
                                &temporary,
                                publish,
                            )
                            .map_err(|error| error.to_string())?;
                            Ok("Installed Minecraft 26.3 from the selected folder.".into())
                        })();
                        if env.exception_check().unwrap_or(false) {
                            let _ = env.exception_clear();
                        }
                        let _ = fs::remove_dir_all(&temporary);
                        result
                    }
                    Source::Archive => {
                        let opened = crate::document_bridge::open(
                            &mut env,
                            activity.as_obj(),
                            &uri,
                            "minecraft.zip",
                        );
                        if opened.is_err() {
                            let _ = env.exception_clear();
                        }
                        let (file, name) = opened.map_err(|error| error.to_string())?;
                        set_detail(format!("Extracting {name}..."));
                        official_install::install_from_archive(&request.home, file, &name, publish)
                            .map_err(|error| error.to_string())?;
                        Ok(format!("Installed Minecraft 26.3 from {name}."))
                    }
                }
            })();
            finish(result);
        });
    if let Err(error) = spawned {
        finish(Err(format!("Could not start the import: {error}")));
    }
}

fn set_detail(text: String) {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).detail = Some(text);
}

fn publish(progress: InstallProgress) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if progress.stage != InstallStage::Metadata {
        state.detail = None;
    }
    state.progress = progress;
}

fn finish(result: std::result::Result<String, String>) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.working = false;
    state.detail = None;
    if result.is_err() {
        state.progress = InstallProgress {
            stage: InstallStage::Metadata,
            done: 0,
            total: 0,
        };
    }
    state.result = Some(result);
}

fn temporary_directory() -> std::result::Result<PathBuf, String> {
    let base = crate::platform::cache_dir()
        .ok_or("Could not access the Android import cache")?
        .join("minecraft-imports");
    fs::create_dir_all(&base).map_err(|error| error.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path = base.join(format!("folder-{}-{stamp}", std::process::id()));
    fs::create_dir(&path).map_err(|error| error.to_string())?;
    Ok(path)
}
