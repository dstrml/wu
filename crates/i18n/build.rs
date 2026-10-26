use std::{
    collections::BTreeMap,
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

const LANGUAGES: &[(&str, &str)] = &[("zh-CN", "SIMPLIFIED_CHINESE")];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionText {
    name: String,
    #[serde(default)]
    description: Option<String>,
}

fn main() {
    let locales = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"))
        .join("../../assets/locales");
    println!("cargo:rerun-if-changed={}", locales.display());

    let mut output = String::new();
    for (code, name) in LANGUAGES {
        let directory = locales.join(code);
        let strings = read_maps::<String>(&directory.join("strings"));
        let actions = read_maps::<ActionText>(&directory.join("actions"));
        let namespaces = read_map::<String>(&directory.join("action_namespaces.json"));
        let schema_descriptions = read_maps::<String>(&directory.join("schema"));

        writeln!(output, "pub(crate) static {name}: Catalog = Catalog {{").ok();
        output.push_str("    strings: &[\n");
        for (key, value) in &strings {
            writeln!(output, "        ({key:?}, {value:?}),").ok();
        }
        output.push_str("    ],\n    actions: &[\n");
        for (key, action) in &actions {
            writeln!(
                output,
                "        ({key:?}, {:?}, {:?}),",
                action.name, action.description
            )
            .ok();
        }
        output.push_str("    ],\n    action_namespaces: &[\n");
        for (key, value) in &namespaces {
            writeln!(output, "        ({key:?}, {value:?}),").ok();
        }
        output.push_str("    ],\n    schema_descriptions: &[\n");
        for (key, value) in &schema_descriptions {
            writeln!(output, "        ({key:?}, {value:?}),").ok();
        }
        output.push_str("    ],\n};\n");
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set"));
    fs::write(out_dir.join("catalogs.rs"), output).expect("writing catalogs.rs");
}

fn read_maps<T: for<'de> Deserialize<'de>>(directory: &Path) -> BTreeMap<String, T> {
    let mut paths = fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(|entry| Some(entry.ok()?.path()))
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "json")
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    paths.sort();
    let mut merged = BTreeMap::new();
    for path in paths {
        for (key, value) in read_map(&path) {
            merged.entry(key).or_insert(value);
        }
    }
    merged
}

fn read_map<T: for<'de> Deserialize<'de>>(path: &Path) -> BTreeMap<String, T> {
    let Ok(contents) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("parsing {}: {error}", path.display()))
}
