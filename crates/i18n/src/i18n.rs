use std::{
    fmt::Display,
    iter,
    sync::atomic::{AtomicU8, AtomicU64, Ordering},
};

pub use gpui_shared_string::SharedString;

/// Translates a string literal into the current UI language, falling back to the English text.
///
/// ```ignore
/// t!("Save");
/// t!("{count} files changed", count = changed.len());
/// ```
#[macro_export]
macro_rules! t {
    ($text:literal $(,)?) => {{
        static CALL_SITE: $crate::CallSite = $crate::CallSite::new();
        CALL_SITE.translate($text)
    }};
    ($text:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        static CALL_SITE: $crate::CallSite = $crate::CallSite::new();
        $crate::format(
            CALL_SITE.translate($text),
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+],
        )
    }};
}

/// Like [`t!`], with a short context for English text that needs different translations in
/// different places.
///
/// ```ignore
/// tc!("git", "Stage");
/// ```
#[macro_export]
macro_rules! tc {
    ($context:literal, $text:literal $(,)?) => {{
        static CALL_SITE: $crate::CallSite = $crate::CallSite::new();
        CALL_SITE.translate_in_context($context, $text)
    }};
    ($context:literal, $text:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        static CALL_SITE: $crate::CallSite = $crate::CallSite::new();
        $crate::format(
            CALL_SITE.translate_in_context($context, $text),
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+],
        )
    }};
}

/// Marks English text that is stored now and translated with [`translate`] where it's shown.
#[macro_export]
macro_rules! n {
    ($text:literal $(,)?) => {
        $text
    };
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Language {
    #[default]
    English = 0,
    SimplifiedChinese = 1,
    /// Wraps every translated string in brackets, to find text that isn't translated yet.
    Pseudo = 2,
}

impl Language {
    pub fn from_locale(locale: &str) -> Self {
        if locale.to_ascii_lowercase().starts_with("zh") {
            Self::SimplifiedChinese
        } else {
            Self::English
        }
    }

    pub fn system() -> Self {
        system_locale().map_or(Self::English, |locale| Self::from_locale(&locale))
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
            Self::Pseudo => "pseudo",
        }
    }

    fn catalog(self) -> Option<&'static Catalog> {
        match self {
            Self::SimplifiedChinese => Some(&SIMPLIFIED_CHINESE),
            Self::English | Self::Pseudo => None,
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

// Wu sets its own `AppleLanguages`, which `sys_locale` would report back, so read the global one.
#[cfg(target_os = "macos")]
fn system_locale() -> Option<String> {
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        string::CFString,
    };
    use core_foundation_sys::preferences::{
        CFPreferencesCopyValue, kCFPreferencesAnyApplication, kCFPreferencesAnyHost,
        kCFPreferencesCurrentUser,
    };

    let key = CFString::from_static_string("AppleLanguages");
    let global_languages = unsafe {
        let value = CFPreferencesCopyValue(
            key.as_concrete_TypeRef(),
            kCFPreferencesAnyApplication,
            kCFPreferencesCurrentUser,
            kCFPreferencesAnyHost,
        );
        (!value.is_null()).then(|| CFType::wrap_under_create_rule(value))
    };
    global_languages
        .and_then(|value| value.downcast_into::<CFArray>())
        .and_then(|languages| {
            let first = languages.get(0)?;
            unsafe { CFType::wrap_under_get_rule(*first) }.downcast_into::<CFString>()
        })
        .map(|language| language.to_string())
        .or_else(sys_locale::get_locale)
}

static LANGUAGE: AtomicU8 = AtomicU8::new(Language::English as u8);

pub fn language() -> Language {
    match LANGUAGE.load(Ordering::Relaxed) {
        1 => Language::SimplifiedChinese,
        2 => Language::Pseudo,
        _ => Language::English,
    }
}

pub fn set_language(language: Language) {
    LANGUAGE.store(language as u8, Ordering::Relaxed);
}

/// Returns the current language's text for `english`, or `None` when it should show in English.
pub fn lookup(english: &str) -> Option<SharedString> {
    let language = language();
    if language == Language::Pseudo {
        return Some(pseudo(english));
    }
    let catalog = language.catalog()?;
    let index = catalog.find_string(english)?;
    Some(SharedString::new_static(catalog.strings[index].1))
}

pub fn translate(english: &'static str) -> SharedString {
    lookup(english).unwrap_or_else(|| SharedString::new_static(english))
}

pub fn translate_in_context(context: &'static str, english: &'static str) -> SharedString {
    CallSite::new().translate_in_context(context, english)
}

/// Remembers one `t!` or `tc!` call's translation, so repeated renders skip the table search.
#[doc(hidden)]
pub struct CallSite(AtomicU64);

const NOT_TRANSLATED: u32 = u32::MAX;

impl CallSite {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn translate(&self, english: &'static str) -> SharedString {
        self.resolve(english, |catalog| catalog.find_string(english))
    }

    pub fn translate_in_context(
        &self,
        context: &'static str,
        english: &'static str,
    ) -> SharedString {
        self.resolve(english, |catalog| {
            catalog.find_string_in_context(context, english)
        })
    }

    fn resolve(
        &self,
        english: &'static str,
        find: impl FnOnce(&'static Catalog) -> Option<usize>,
    ) -> SharedString {
        let language = language();
        let Some(catalog) = language.catalog() else {
            return match language {
                Language::Pseudo => pseudo(english),
                Language::English | Language::SimplifiedChinese => {
                    SharedString::new_static(english)
                }
            };
        };
        let tag = language as u64 + 1;
        let cached = self.0.load(Ordering::Relaxed);
        let index = if cached >> 32 == tag {
            cached as u32
        } else {
            let index = find(catalog)
                .and_then(|index| u32::try_from(index).ok())
                .unwrap_or(NOT_TRANSLATED);
            self.0
                .store(tag << 32 | u64::from(index), Ordering::Relaxed);
            index
        };
        match catalog.strings.get(index as usize) {
            Some((_, translation)) => SharedString::new_static(translation),
            None => SharedString::new_static(english),
        }
    }
}

impl Default for CallSite {
    fn default() -> Self {
        Self::new()
    }
}

/// Fills `{name}` placeholders in `template`. Unknown placeholders are left as they are.
pub fn format(template: SharedString, arguments: &[(&str, &dyn Display)]) -> SharedString {
    let mut result = String::with_capacity(template.len());
    let mut rest = template.as_str();
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let after_brace = &rest[start + 1..];
        let argument = after_brace.find('}').and_then(|end| {
            let name = &after_brace[..end];
            arguments
                .iter()
                .find(|(argument_name, _)| *argument_name == name)
                .map(|(_, value)| (value, end))
        });
        match argument {
            Some((value, end)) => {
                result.push_str(&value.to_string());
                rest = &after_brace[end + 1..];
            }
            None => {
                result.push('{');
                rest = after_brace;
            }
        }
    }
    result.push_str(rest);
    result.into()
}

/// The translated name of a command, like "切换注释" for `editor::ToggleComments`.
pub fn action_name(action: &str) -> Option<SharedString> {
    let language = language();
    if language == Language::Pseudo {
        return Some(pseudo(action));
    }
    let &(_, name, _) = language.catalog()?.find_action(action)?;
    Some(SharedString::new_static(name))
}

/// The translated name of a command namespace, like "编辑器" for `editor`.
pub fn action_namespace(namespace: &str) -> Option<SharedString> {
    let language = language();
    if language == Language::Pseudo {
        return Some(pseudo(namespace));
    }
    let namespaces = language.catalog()?.action_namespaces;
    let index = namespaces
        .binary_search_by(|(key, _)| (*key).cmp(namespace))
        .ok()?;
    Some(SharedString::new_static(namespaces[index].1))
}

pub fn action_description(action: &str) -> Option<SharedString> {
    let language = language();
    if language == Language::Pseudo {
        return Some(pseudo(action));
    }
    let &(_, _, description) = language.catalog()?.find_action(action)?;
    description.map(SharedString::new_static)
}

/// The translated description of a setting or command for the JSON schema, used by the hover
/// help in settings.json and keymap.json.
pub fn schema_description(english: &str) -> Option<SharedString> {
    let language = language();
    if language == Language::Pseudo {
        return Some(pseudo(english));
    }
    let descriptions = language.catalog()?.schema_descriptions;
    let index = descriptions
        .binary_search_by(|(key, _)| (*key).cmp(english))
        .ok()?;
    Some(SharedString::new_static(descriptions[index].1))
}

/// Whether `text` is `english` or its translation in any language.
pub fn is_any_translation_of(text: &str, english: &str) -> bool {
    text == english
        || [&SIMPLIFIED_CHINESE].into_iter().any(|catalog| {
            catalog
                .find_string(english)
                .is_some_and(|index| catalog.strings[index].1 == text)
        })
}

/// Translates English text that was built from one of `templates`, keeping the values that filled
/// its `{name}` placeholders. Returns `None` when no template matches.
pub fn translate_filled(text: &str, templates: &[&'static str]) -> Option<SharedString> {
    templates.iter().find_map(|template| {
        let values = match_template(template, text)?;
        let arguments = values
            .iter()
            .map(|(name, value)| (*name, value as &dyn Display))
            .collect::<Vec<_>>();
        Some(format(translate(template), &arguments))
    })
}

fn match_template<'a>(template: &'a str, text: &'a str) -> Option<Vec<(&'a str, &'a str)>> {
    let mut values = Vec::new();
    let mut template_rest = template;
    let mut text_rest = text;
    loop {
        let Some(start) = template_rest.find('{') else {
            return (template_rest == text_rest).then_some(values);
        };
        let literal = &template_rest[..start];
        text_rest = text_rest.strip_prefix(literal)?;
        let end = template_rest[start..].find('}')? + start;
        let name = &template_rest[start + 1..end];
        template_rest = &template_rest[end + 1..];
        let next_literal_end = template_rest.find('{').unwrap_or(template_rest.len());
        let next_literal = &template_rest[..next_literal_end];
        let value_end = if next_literal.is_empty() {
            if template_rest.is_empty() {
                text_rest.len()
            } else {
                return None;
            }
        } else if next_literal_end == template_rest.len() {
            text_rest.strip_suffix(next_literal)?.len()
        } else {
            text_rest.find(next_literal)?
        };
        values.push((name, &text_rest[..value_end]));
        text_rest = &text_rest[value_end..];
    }
}

fn pseudo(english: &str) -> SharedString {
    format!("⟦{english}⟧").into()
}

/// Tables generated from `assets/locales` by build.rs, each sorted by key.
struct Catalog {
    strings: &'static [(&'static str, &'static str)],
    actions: &'static [(&'static str, &'static str, Option<&'static str>)],
    action_namespaces: &'static [(&'static str, &'static str)],
    schema_descriptions: &'static [(&'static str, &'static str)],
}

impl Catalog {
    fn find_string(&self, english: &str) -> Option<usize> {
        self.strings
            .binary_search_by(|(key, _)| (*key).cmp(english))
            .ok()
    }

    fn find_string_in_context(&self, context: &str, english: &str) -> Option<usize> {
        self.strings
            .binary_search_by(|(key, _)| {
                key.bytes().cmp(
                    context
                        .bytes()
                        .chain(iter::once(b'|'))
                        .chain(english.bytes()),
                )
            })
            .ok()
    }

    fn find_action(
        &self,
        action: &str,
    ) -> Option<&'static (&'static str, &'static str, Option<&'static str>)> {
        let index = self
            .actions
            .binary_search_by(|(key, _, _)| (*key).cmp(action))
            .ok()?;
        self.actions.get(index)
    }
}

include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use serde::Deserialize;
    use std::{
        collections::{BTreeMap, BTreeSet},
        path::{Path, PathBuf},
        sync::Mutex,
    };

    static LANGUAGE_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ActionText {
        name: String,
        #[serde(default)]
        description: Option<String>,
    }

    fn context_key(context: &str, english: &str) -> String {
        format!("{context}|{english}")
    }

    fn with_language<R>(language: Language, test: impl FnOnce() -> R) -> R {
        let _guard = LANGUAGE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set_language(language);
        let result = test();
        set_language(Language::English);
        result
    }

    #[test]
    fn english_returns_the_source_text() {
        with_language(Language::English, || {
            assert_eq!(t!("Save").as_str(), "Save");
            assert_eq!(tc!("git", "Stage").as_str(), "Stage");
            assert_eq!(
                t!("{count} files changed", count = 3).as_str(),
                "3 files changed"
            );
            assert_eq!(action_name("editor::ToggleComments"), None);
        });
    }

    #[test]
    fn missing_translations_fall_back_to_english() {
        with_language(Language::SimplifiedChinese, || {
            assert_eq!(
                t!("This text has no translation, on purpose").as_str(),
                "This text has no translation, on purpose"
            );
            assert_eq!(action_name("test::NoSuchCommand"), None);
        });
    }

    #[test]
    fn simplified_chinese_uses_the_generated_tables() {
        with_language(Language::SimplifiedChinese, || {
            assert_eq!(t!("Rename").as_str(), "重命名");
            assert_eq!(tc!("chat", "Pin").as_str(), "置顶");
            assert_eq!(translate("Rename").as_str(), "重命名");
            assert_eq!(lookup("Rename").as_deref(), Some("重命名"));
            assert_eq!(
                action_name("editor::ToggleComments").as_deref(),
                Some("切换注释")
            );
            assert!(action_description("editor::ToggleComments").is_some());
            assert_eq!(action_namespace("editor").as_deref(), Some("编辑器"));
        });
    }

    #[test]
    fn templates_match_filled_english_text() {
        assert_eq!(
            match_template("Forked from \"{title}\"", "Forked from \"My chat\""),
            Some(vec![("title", "My chat")])
        );
        assert_eq!(
            match_template("{count} of {total} done", "3 of 5 done"),
            Some(vec![("count", "3"), ("total", "5")])
        );
        assert_eq!(
            match_template("Side chat of {title}", "Side chat of "),
            Some(vec![("title", "")])
        );
        assert_eq!(
            match_template("Forked from \"{title}\"", "Something else"),
            None
        );
        assert_eq!(match_template("{a}{b}", "xy"), None);
        assert_eq!(
            match_template("No placeholders", "No placeholders"),
            Some(vec![])
        );
        with_language(Language::English, || {
            assert_eq!(
                translate_filled(
                    "Forked from \"X\"",
                    &["Side chat of {title}", "Forked from \"{title}\""]
                )
                .as_deref(),
                Some("Forked from \"X\"")
            );
            assert_eq!(
                translate_filled("Unrelated", &["Forked from \"{title}\""]),
                None
            );
        });
    }

    #[test]
    fn titles_are_recognized_in_every_language() {
        assert!(is_any_translation_of("Rename", "Rename"));
        assert!(is_any_translation_of("重命名", "Rename"));
        assert!(!is_any_translation_of("删除", "Rename"));
    }

    #[test]
    fn call_site_cache_follows_language_changes() {
        fn rename_label() -> SharedString {
            t!("Rename")
        }
        let _guard = LANGUAGE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut labels = Vec::new();
        for language in [
            Language::SimplifiedChinese,
            Language::SimplifiedChinese,
            Language::English,
            Language::Pseudo,
            Language::SimplifiedChinese,
        ] {
            set_language(language);
            labels.push(rename_label().to_string());
        }
        set_language(Language::English);
        assert_eq!(labels, ["重命名", "重命名", "Rename", "⟦Rename⟧", "重命名"]);
    }

    #[test]
    fn generated_tables_are_sorted_and_unique() {
        let catalog = &SIMPLIFIED_CHINESE;
        assert!(!catalog.strings.is_empty());
        assert!(catalog.strings.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(catalog.actions.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(
            catalog
                .action_namespaces
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0)
        );
        assert!(
            catalog
                .schema_descriptions
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0)
        );
    }

    #[test]
    fn pseudo_language_marks_every_string() {
        with_language(Language::Pseudo, || {
            assert_eq!(t!("Save").as_str(), "⟦Save⟧");
            assert_eq!(
                t!("{count} files changed", count = 3).as_str(),
                "⟦3 files changed⟧"
            );
        });
    }

    #[test]
    fn format_fills_placeholders_in_any_order() {
        let template = SharedString::new_static("{name} 已移动到 {target}，共 {count} 项");
        assert_eq!(
            format(
                template,
                &[("count", &2), ("target", &"src"), ("name", &"main.rs")]
            )
            .as_str(),
            "main.rs 已移动到 src，共 2 项"
        );
        assert_eq!(
            format(SharedString::new_static("{missing} {"), &[("other", &1)]).as_str(),
            "{missing} {"
        );
    }

    #[test]
    fn locales_map_to_languages() {
        for locale in [
            "zh",
            "zh-CN",
            "zh-Hans",
            "zh-Hans-CN",
            "zh-SG",
            "zh-TW",
            "zh-HK",
            "ZH_cn",
        ] {
            assert_eq!(
                Language::from_locale(locale),
                Language::SimplifiedChinese,
                "{locale}"
            );
        }
        for locale in ["en", "en-US", "fr-FR", "ja-JP", ""] {
            assert_eq!(Language::from_locale(locale), Language::English, "{locale}");
        }
    }

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn read_json_files<T: serde::de::DeserializeOwned>(directory: &Path) -> Vec<(PathBuf, T)> {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return Vec::new();
        };
        let mut paths = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                let contents = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
                let parsed = serde_json::from_str(&contents)
                    .unwrap_or_else(|error| panic!("parsing {}: {error}", path.display()));
                (path, parsed)
            })
            .collect()
    }

    fn unescape_rust_string(literal: &str) -> String {
        let mut result = String::with_capacity(literal.len());
        let mut characters = literal.chars().peekable();
        while let Some(character) = characters.next() {
            if character != '\\' {
                result.push(character);
                continue;
            }
            match characters.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('0') => result.push('\0'),
                Some('u') => {
                    let code = characters
                        .by_ref()
                        .skip_while(|character| *character == '{')
                        .take_while(|character| *character != '}')
                        .collect::<String>();
                    if let Some(character) =
                        u32::from_str_radix(&code, 16).ok().and_then(char::from_u32)
                    {
                        result.push(character);
                    }
                }
                Some('\n') => {
                    while characters
                        .peek()
                        .is_some_and(|character| character.is_whitespace())
                    {
                        characters.next();
                    }
                }
                Some(other) => result.push(other),
                None => {}
            }
        }
        result
    }

    fn source_keys() -> BTreeMap<String, BTreeSet<PathBuf>> {
        let string = r#""((?s:[^"\\]|\\.)*)""#;
        let plain = Regex::new(&format!(r"\b(?:t|n)!\(\s*{string}")).unwrap();
        let contextual = Regex::new(&format!(r"\btc!\(\s*{string}\s*,\s*{string}")).unwrap();
        let crates = repository_root().join("crates");
        let mut keys = BTreeMap::<String, BTreeSet<PathBuf>>::new();
        for entry in walkdir::WalkDir::new(&crates)
            .into_iter()
            .filter_entry(|entry| {
                let name = entry.file_name();
                name != "target" && entry.path() != crates.join("i18n")
            })
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(path) else {
                continue;
            };
            let relative_path = path.strip_prefix(&crates).unwrap_or(path).to_path_buf();
            for captures in plain.captures_iter(&source) {
                keys.entry(unescape_rust_string(&captures[1]))
                    .or_default()
                    .insert(relative_path.clone());
            }
            for captures in contextual.captures_iter(&source) {
                let key = context_key(
                    &unescape_rust_string(&captures[1]),
                    &unescape_rust_string(&captures[2]),
                );
                keys.entry(key).or_default().insert(relative_path.clone());
            }
        }
        keys
    }

    fn placeholders(text: &str) -> BTreeSet<String> {
        let placeholder = Regex::new(r"\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap();
        placeholder
            .captures_iter(text)
            .map(|captures| captures[1].to_string())
            .collect()
    }

    fn english_text(key: &str) -> &str {
        key.split_once('|').map_or(key, |(_, english)| english)
    }

    #[test]
    fn simplified_chinese_strings_are_complete_and_consistent() {
        let directory = repository_root().join("assets/locales/zh-CN/strings");
        let mut translations = BTreeMap::<String, (String, PathBuf)>::new();
        let mut problems = Vec::new();

        for (path, file) in read_json_files::<BTreeMap<String, String>>(&directory) {
            for (key, chinese) in file {
                if chinese.trim().is_empty() {
                    problems.push(format!("{}: {key:?} is empty", path.display()));
                }
                if placeholders(english_text(&key)) != placeholders(&chinese) {
                    problems.push(format!(
                        "{}: {key:?} and {chinese:?} have different placeholders",
                        path.display()
                    ));
                }
                if let Some((existing, existing_path)) = translations.get(&key)
                    && *existing != chinese
                {
                    problems.push(format!(
                        "{key:?} is {existing:?} in {} but {chinese:?} in {}; use tc! to tell them apart",
                        existing_path.display(),
                        path.display()
                    ));
                }
                translations.insert(key, (chinese, path.clone()));
            }
        }

        let used = source_keys();
        for (key, files) in &used {
            if !translations.contains_key(key) {
                let files = files
                    .iter()
                    .map(|file| file.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                problems.push(format!(
                    "{key:?} has no Chinese translation (used in {files})"
                ));
            }
        }
        for (key, (_, path)) in &translations {
            if !used.contains_key(key) {
                problems.push(format!("{}: {key:?} is not used anywhere", path.display()));
            }
        }

        assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    }

    const ACTIONS_MISSING_FROM_MACOS_RELEASE: &[&str] = &[
        "dev::ShowWorkspaceError",
        "projects::OpenFolderInWsl",
        "projects::OpenWsl",
    ];

    #[derive(Deserialize)]
    struct DumpedActions {
        actions: Vec<DumpedAction>,
    }

    #[derive(Deserialize)]
    struct DumpedAction {
        name: String,
        documentation: Option<String>,
    }

    /// Runs only with `WU_ACTIONS_DUMP` set to the output of `wu --dump-all-actions`.
    #[test]
    fn simplified_chinese_commands_are_complete() {
        let Some(dump_path) = std::env::var_os("WU_ACTIONS_DUMP") else {
            return;
        };
        let dump = std::fs::read_to_string(&dump_path)
            .unwrap_or_else(|error| panic!("reading {dump_path:?}: {error}"));
        let dump: DumpedActions = serde_json::from_str(&dump)
            .unwrap_or_else(|error| panic!("parsing {dump_path:?}: {error}"));

        let locales = repository_root().join("assets/locales/zh-CN");
        let mut translations = BTreeMap::new();
        for (_, file) in read_json_files::<BTreeMap<String, ActionText>>(&locales.join("actions")) {
            translations.extend(file);
        }
        let namespaces = read_json_files::<BTreeMap<String, String>>(&locales)
            .into_iter()
            .filter(|(path, _)| path.ends_with("action_namespaces.json"))
            .flat_map(|(_, file)| file.into_keys())
            .collect::<BTreeSet<_>>();

        let mut problems = Vec::new();
        let mut registered = BTreeSet::new();
        for action in &dump.actions {
            registered.insert(action.name.as_str());
            if let Some((namespace, _)) = action.name.split_once("::")
                && !namespaces.contains(namespace)
            {
                problems.push(format!("namespace {namespace:?} has no Chinese name"));
            }
            match translations.get(&action.name) {
                None => problems.push(format!("{} has no Chinese name", action.name)),
                Some(translation) if translation.name.trim().is_empty() => {
                    problems.push(format!("{} has an empty Chinese name", action.name))
                }
                Some(translation) => {
                    if action.documentation.is_some() && translation.description.is_none() {
                        problems.push(format!("{} has no Chinese description", action.name));
                    }
                }
            }
        }
        for name in translations.keys() {
            if !registered.contains(name.as_str())
                && !ACTIONS_MISSING_FROM_MACOS_RELEASE.contains(&name.as_str())
            {
                problems.push(format!("{name} is not a registered command"));
            }
        }
        problems.sort();
        problems.dedup();
        assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    }
}
