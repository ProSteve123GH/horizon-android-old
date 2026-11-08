//! Recursive SAF tree copy for local Minecraft imports.
//!
//! A folder grant gives no filesystem path. Walk `DocumentsContract` children and
//! copy only files the installer can use: a client JAR, the 26.3 asset index and
//! hash-named asset objects. Nothing outside the granted tree is read.
use horizon_client::Result;
use jni::{
    JNIEnv,
    objects::{GlobalRef, JObject, JValue},
};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

const MAX_FILES: usize = 200_000;
const MAX_DEPTH: usize = 32;
const MAX_FILE_BYTES: u64 = 1 << 30;
const MAX_TOTAL_BYTES: u64 = 8 << 30;
const MAX_NAME: usize = 240;

struct Child {
    id: String,
    name: String,
    directory: bool,
}

pub fn copy(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    tree_uri: &str,
    destination: &Path,
    progress: &mut dyn FnMut(u64),
) -> Result<()> {
    let (resolver, tree, root) = env
        .with_local_frame(24, |env| -> jni::errors::Result<_> {
            let resolver = env
                .call_method(
                    activity,
                    "getContentResolver",
                    "()Landroid/content/ContentResolver;",
                    &[],
                )?
                .l()?;
            let text = env.new_string(tree_uri)?;
            let uri = env
                .call_static_method(
                    "android/net/Uri",
                    "parse",
                    "(Ljava/lang/String;)Landroid/net/Uri;",
                    &[JValue::Object(&JObject::from(text))],
                )?
                .l()?;
            let root = env
                .call_static_method(
                    "android/provider/DocumentsContract",
                    "getTreeDocumentId",
                    "(Landroid/net/Uri;)Ljava/lang/String;",
                    &[JValue::Object(&uri)],
                )?
                .l()?;
            if root.is_null() {
                return Err(jni::errors::Error::NullPtr(
                    "Selected folder has no document id",
                ));
            }
            let root: String = env.get_string((&root).into())?.into();
            Ok((
                env.new_global_ref(resolver)?,
                env.new_global_ref(uri)?,
                root,
            ))
        })
        .map_err(|error| format!("Could not open the selected folder: {error}"))?;
    let mut stack = vec![(root, PathBuf::new(), 0usize)];
    let mut seen = HashSet::new();
    let mut files = 0usize;
    let mut total = 0u64;
    let mut reported = 0u64;
    while let Some((document, relative, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err("Selected folder is nested too deeply".into());
        }
        for child in list_children(env, &resolver, &tree, &document)? {
            let name = safe_name(&child.name)?;
            let mut path = relative.clone();
            path.push(&name);
            if child.directory {
                if !ignored_dir(&name) {
                    stack.push((child.id, path, depth + 1));
                }
                continue;
            }
            if !wanted(&name, &path) {
                continue;
            }
            if !seen.insert(path.clone()) {
                return Err("Selected folder contains duplicate names".into());
            }
            files += 1;
            if files > MAX_FILES {
                return Err("Selected folder contains too many Minecraft files".into());
            }
            let uri = document_uri(env, &tree, &child.id)?;
            let opened = crate::document_bridge::open(env, activity, &uri, &name);
            if opened.is_err() {
                let _ = env.exception_clear();
            }
            let (mut input, _) = opened.map_err(|error| error.to_string())?;
            let target = destination.join(&path);
            fs::create_dir_all(target.parent().ok_or("Missing folder import parent")?)?;
            let mut output = fs::File::create(&target)?;
            let copied = copy_bounded(&mut input, &mut output, MAX_FILE_BYTES)?;
            total = total
                .checked_add(copied)
                .filter(|total| *total <= MAX_TOTAL_BYTES)
                .ok_or("Selected Minecraft files exceed 8 GiB")?;
            if total - reported >= 8 << 20 {
                reported = total;
                progress(total);
            }
        }
    }
    progress(total);
    Ok(())
}

fn list_children(
    env: &mut JNIEnv<'_>,
    resolver: &GlobalRef,
    tree: &GlobalRef,
    document: &str,
) -> Result<Vec<Child>> {
    env.with_local_frame(32, |env| -> jni::errors::Result<Vec<Child>> {
        let id = env.new_string(document)?;
        let uri = env
            .call_static_method(
                "android/provider/DocumentsContract",
                "buildChildDocumentsUriUsingTree",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                &[
                    JValue::Object(tree.as_obj()),
                    JValue::Object(&JObject::from(id)),
                ],
            )?
            .l()?;
        let projection = env.new_object_array(3, "java/lang/String", JObject::null())?;
        for (index, column) in ["document_id", "_display_name", "mime_type"]
            .iter()
            .enumerate()
        {
            env.set_object_array_element(
                &projection,
                index as i32,
                env.new_string(*column)?,
            )?;
        }
        let null = JObject::null();
        let cursor = env
            .call_method(
                resolver.as_obj(),
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                &[
                    JValue::Object(&uri),
                    JValue::Object(projection.as_ref()),
                    JValue::Object(&null),
                    JValue::Object(&null),
                    JValue::Object(&null),
                ],
            )?
            .l()?;
        if cursor.is_null() {
            return Err(jni::errors::Error::NullPtr(
                "Document provider returned no folder listing",
            ));
        }
        let result = (|| -> jni::errors::Result<Vec<Child>> {
            let count = env.call_method(&cursor, "getCount", "()I", &[])?.i()?;
            let mut children = Vec::with_capacity(count.max(0) as usize);
            for _ in 0..count {
                if !env.call_method(&cursor, "moveToNext", "()Z", &[])?.z()? {
                    break;
                }
                let (id, name, mime) =
                    env.with_local_frame(8, |env| -> jni::errors::Result<_> {
                        let id = string_at(env, &cursor, 0)?;
                        let name = string_at(env, &cursor, 1)?;
                        let mime = string_at(env, &cursor, 2)?;
                        Ok((id, name, mime))
                    })?;
                children.push(Child {
                    id,
                    name,
                    directory: mime == "vnd.android.document/directory",
                });
            }
            Ok(children)
        })();
        let _ = env.call_method(&cursor, "close", "()V", &[]);
        result
    })
    .map_err(|error| format!("Could not list the selected folder: {error}").into())
}

fn string_at(
    env: &mut JNIEnv<'_>,
    cursor: &JObject<'_>,
    column: i32,
) -> jni::errors::Result<String> {
    let value = env
        .call_method(
            cursor,
            "getString",
            "(I)Ljava/lang/String;",
            &[JValue::Int(column)],
        )?
        .l()?;
    if value.is_null() {
        Ok(String::new())
    } else {
        Ok(env.get_string((&value).into())?.into())
    }
}

fn document_uri(env: &mut JNIEnv<'_>, tree: &GlobalRef, document: &str) -> Result<String> {
    env.with_local_frame(16, |env| -> jni::errors::Result<String> {
        let id = env.new_string(document)?;
        let uri = env
            .call_static_method(
                "android/provider/DocumentsContract",
                "buildDocumentUriUsingTree",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                &[
                    JValue::Object(tree.as_obj()),
                    JValue::Object(&JObject::from(id)),
                ],
            )?
            .l()?;
        let text = env
            .call_method(&uri, "toString", "()Ljava/lang/String;", &[])?
            .l()?;
        Ok(env.get_string((&text).into())?.into())
    })
    .map_err(|error| format!("Could not address a selected file: {error}").into())
}

fn copy_bounded(input: &mut fs::File, output: &mut fs::File, limit: u64) -> Result<u64> {
    let mut buffer = [0; 65536];
    let mut copied = 0u64;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        copied += count as u64;
        if copied > limit {
            return Err("A selected file is larger than 1 GiB".into());
        }
        output.write_all(&buffer[..count])?;
    }
    Ok(copied)
}

fn safe_name(name: &str) -> Result<String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.len() > MAX_NAME
        || name.chars().any(|c| c.is_control())
        || name.contains('/')
        || name.contains('\\')
    {
        return Err("Selected folder contains an unsupported file name".into());
    }
    let path = Path::new(name);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err("Selected folder contains an unsupported file name".into());
    }
    Ok(name.to_owned())
}

fn ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "backups"
            | "cache"
            | "crash-reports"
            | "downloads"
            | "logs"
            | "mods"
            | "resourcepacks"
            | "saves"
            | "screenshots"
            | "shaderpacks"
    )
}

fn is_sha1(name: &str) -> bool {
    name.len() == 40 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn numeric_json(name: &str) -> bool {
    name.strip_suffix(".json")
        .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|byte| byte.is_ascii_digit()))
}

fn likely_client(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower == "client.jar"
        || lower == "26.3.jar"
        || path
            .components()
            .any(|component| component.as_os_str() == "versions")
}

fn wanted(name: &str, path: &Path) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.ends_with(".jar") && likely_client(path))
        || (lower.ends_with(".json")
            && (name == "asset-index.json"
                || numeric_json(name)
                || path
                    .components()
                    .any(|component| component.as_os_str() == "indexes")))
        || is_sha1(name)
}
