//! Legacy GLES lowering. Vulkan retains the original WGSL byte for byte.
use std::borrow::Cow;

pub(super) fn terrain_shader(gl: bool) -> Cow<'static, str> {
    let source = include_str!("../terrain.wgsl");
    if gl {
        Cow::Owned(gles_source(source))
    } else {
        Cow::Borrowed(source)
    }
}

/// GLES 3.0 cannot declare arrays of arrays. Two adjacent vec4 entries
/// retain the original uniform byte layout and every colored-light value.
pub(super) fn gles_uniforms(source: &str) -> String {
    source
        .replace("array<array<vec4<f32>, 2>, 16>", "array<vec4<f32>, 32>")
        .replace(
            "globals.colored_lights[i][0]",
            "globals.colored_lights[2u * i]",
        )
        .replace(
            "globals.colored_lights[i][1]",
            "globals.colored_lights[2u * i + 1u]",
        )
}

fn gles_source(source: &str) -> String {
    let source = gles_uniforms(source).replace("\r\n", "\n");
    let source = unroll_emissive_lookup(source);
    let start = source
        .find("fn sprite_grad(")
        .expect("terrain sprite function");
    let level = source
        .find("fn sprite_level(")
        .expect("terrain level function");
    let end = level + source[level..].find("\n}\n").expect("terrain level end") + 3;
    let rgss = source
        .find("fn sample_rgss(")
        .expect("terrain RGSS function");
    let rgss_end = rgss + source[rgss..].find("\n}\n").expect("terrain RGSS end") + 3;
    let mut output = String::with_capacity(source.len());
    output.push_str(&source[..start]);
    output.push_str(include_str!("terrain_gles.wgsl.inc"));
    output.push_str(&source[end..rgss]);
    output.push_str(&source[rgss_end..]);

    // Adreno 304 V@269's compiler crashes on this local indexed array; the
    // include's four constant taps keep the same samples and accumulation.
    if let Some(start) = output.find("fn rotated_grid(") {
        if let Some(len) = output[start..].find("\n}\n") {
            output.replace_range(start..start + len + 3, "");
        }
    }
    output
}

// Adreno 710's compiler fails to link this dynamic uniform-array search in a
// robust EGL context. Preserve the first match, every entry and the no-match
// sentinel; constant-index branches also keep robust access enabled.
fn unroll_emissive_lookup(mut source: String) -> String {
    let Some(start) =
        source.find("    for (var i = 0u; i < min(globals.emissive_count.x, 128u); i++) {")
    else {
        // The shader and this lowering are separate files; a shader change must
        // not turn every GLES start into a panic. The test below pins the loop
        // this branch expects.
        log::warn!("GLES terrain shader: emissive lookup loop changed; leaving it dynamic");
        return source;
    };
    let body = source[start..].find('{').unwrap() + start;
    let mut depth = 1;
    let mut end = body + 1;
    while depth != 0 {
        match source.as_bytes()[end] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        end += 1;
    }
    // The pairs are sorted by base layer, so a later pair cannot equal
    // `sprite_layer` after the first pair past it; the found guard therefore
    // keeps the loop's first match without its early break. Flat statements
    // keep the 128 branches out of a deeply nested WGSL expression (the
    // validator's stack, and some drivers').
    let mut branches = String::from("    var emissive_found = false;\n");
    for i in 0..128 {
        branches.push_str(&format!(
            "    if !emissive_found && globals.emissive_count.x > {i}u && globals.emissive_layers[{i}].x == sprite_layer {{\n        out.emissive_layer = globals.emissive_layers[{i}].y;\n        emissive_found = true;\n    }}\n"
        ));
    }
    source.replace_range(start..end, &branches);
    source
}

#[cfg(test)]
#[path = "terrain_source/hdr_gpu_tests.rs"]
mod hdr_gpu_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_emissive_loop_leaves_the_source_alone() {
        // A shader edit must not turn every GLES start into a panic.
        let source = "fn sprite_level() {}\n";
        assert_eq!(unroll_emissive_lookup(source.to_owned()), source);
    }

    #[test]
    fn vulkan_source_is_unchanged_and_gles_validates_every_entrypoint() {
        assert!(matches!(terrain_shader(false), Cow::Borrowed(_)));
        assert_eq!(terrain_shader(false), include_str!("../terrain.wgsl"));
        let vulkan = include_str!("../hdr_light.wgsl").to_owned() + terrain_shader(false).as_ref();
        let module = naga::front::wgsl::parse_str(&vulkan).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        // The renderer creates the module from the HDR prelude plus this source.
        let source = include_str!("../hdr_light.wgsl").to_owned() + terrain_shader(true).as_ref();
        let original = include_str!("../terrain.wgsl").replace("\r\n", "\n");
        assert_eq!(
            &source[source.find("struct SkyOut {").unwrap()..],
            &original[original.find("struct SkyOut {").unwrap()..],
            "sky and outline shader bodies must remain unchanged"
        );
        assert!(!source.contains("array<array<"));
        for index in 0..7 {
            assert!(source.contains(&format!("array_index == {index}u")));
        }
        assert!(source.contains("let whole = in.emission > 4u"));
        // The GLES source keeps one definition of the terrain entry points;
        // the old packed-interface copy was invalid WGSL.
        assert_eq!(source.matches("fn opaque_quad(").count(), 1);
        assert_eq!(source.matches("@vertex fn vs_terrain_opaque(").count(), 1);
        // The GLES lowering replaces the 128-entry dynamic lookup with
        // constant-index branches; finding no loop (or only 32 entries) would
        // crash pipeline creation on every GLES device.
        assert!(source.contains("globals.emissive_layers[127].x == sprite_layer"));
        assert!(!source.contains("i < min(globals.emissive_count.x, 128u)"));
        // Adreno 304 crashes on the local indexed RGSS offsets.
        assert!(!source.contains("fn rotated_grid("));
        assert!(!source.contains("uv + offsets[i] * pixel"));
        let module = naga::front::wgsl::parse_str(&source).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        let vertex = module
            .entry_points
            .iter()
            .find(|e| e.name == "vs_terrain")
            .unwrap();
        let naga::TypeInner::Struct { members, .. } =
            &module.types[vertex.function.result.as_ref().unwrap().ty].inner
        else {
            panic!("terrain result must be a struct")
        };
        assert_eq!(
            members
                .iter()
                .filter(|m| matches!(m.binding, Some(naga::Binding::Location { .. })))
                .count(),
            5
        );
    }

    #[test]
    fn windows_line_endings_produce_the_same_gles_shader() {
        let source = include_str!("../terrain.wgsl").replace("\r\n", "\n");
        assert_eq!(
            gles_source(&source),
            gles_source(&source.replace('\n', "\r\n"))
        );
    }
}
