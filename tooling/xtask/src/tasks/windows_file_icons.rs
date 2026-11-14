use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{Context as _, Result, anyhow};
use clap::Parser;
use regex::Regex;
use resvg::{tiny_skia, usvg};
use serde::Deserialize;

use crate::workspace::load_workspace;

const SYMBOLS_THEME: &str = "assets/icon_themes/symbols/icon_theme.json";
const SYMBOLS_THEME_NAME: &str = "Symbols Light";
const LOGO: &str = "assets/images/wu_logo.svg";
const INSTALLER_SCRIPT: &str = "crates/wu/resources/windows/zed.iss";
const OUTPUT_DIR: &str = "crates/wu/resources/windows/file_icons";
const INSTALLED_DIR: &str = r"{app}\resources\file_icons";
const GENERIC_ICON: &str = "document";
const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];
const SMALLEST_SIZE_WITH_BADGE: u32 = 32;

const EXTRA_ICONS: &[(&str, &str)] = &[
    ("bash_login", "shell"),
    ("bash_logout", "shell"),
    ("bash_profile", "shell"),
    ("bashrc", "shell"),
    ("profile", "shell"),
    ("cljx", "clojure"),
    ("clojure", "clojure"),
    ("containerfile", "docker"),
    ("eyaml", "yaml"),
    ("eyml", "yaml"),
    ("fsscript", "fsharp"),
    ("h++", "cplus"),
    ("jav", "java"),
    ("mdown", "markdown"),
    ("mdtext", "markdown"),
    ("mdtxt", "markdown"),
    ("mdwn", "markdown"),
    ("mkd", "markdown"),
    ("mkdn", "markdown"),
    ("ctp", "php"),
    ("phtml", "php"),
    ("pl6", "perl"),
    ("pm6", "perl"),
    ("pod", "perl"),
    ("psgi", "perl"),
    ("t", "perl"),
    ("rprofile", "r"),
    ("code-workspace", "brackets-yellow"),
    ("bowerrc", "brackets-yellow"),
    ("jscsrc", "brackets-yellow"),
    ("jshintrc", "brackets-yellow"),
    ("config", "xml"),
    ("wxi", "xml"),
    ("wxl", "xml"),
    ("wxs", "xml"),
    ("xaml", "xml"),
    ("log", "text"),
];

#[derive(Parser)]
pub struct WindowsFileIconsArgs {}

#[derive(Deserialize)]
struct IconThemeFamily {
    themes: Vec<IconTheme>,
}

#[derive(Deserialize)]
struct IconTheme {
    name: String,
    file_stems: BTreeMap<String, String>,
    file_suffixes: BTreeMap<String, String>,
    file_icons: BTreeMap<String, IconDefinition>,
}

#[derive(Deserialize)]
struct IconDefinition {
    path: String,
}

pub fn run_windows_file_icons(_args: WindowsFileIconsArgs) -> Result<()> {
    let workspace = load_workspace()?;
    let root = workspace.workspace_root.as_std_path();

    let theme_family: IconThemeFamily =
        serde_json_lenient::from_str(&fs::read_to_string(root.join(SYMBOLS_THEME))?)
            .context("failed to parse the Symbols icon theme")?;
    let theme = theme_family
        .themes
        .into_iter()
        .find(|theme| theme.name == SYMBOLS_THEME_NAME)
        .with_context(|| format!("icon theme {SYMBOLS_THEME_NAME:?} not found"))?;

    let installer_path = root.join(INSTALLER_SCRIPT);
    let installer = fs::read_to_string(&installer_path)?;
    let extension_pattern = Regex::new(
        r#"Subkey: "Software\\Classes\\\.([^\\"]+)\\OpenWithProgids"; ValueType: none"#,
    )?;
    let icon_by_extension: BTreeMap<String, String> = extension_pattern
        .captures_iter(&installer)
        .map(|captures| {
            let extension = captures[1].to_string();
            let icon = icon_for_extension(&extension, &theme);
            (extension, icon)
        })
        .collect();
    if icon_by_extension.is_empty() {
        return Err(anyhow!("no file associations found in {INSTALLER_SCRIPT}"));
    }

    let logo = svg_body(&fs::read_to_string(root.join(LOGO))?)?;
    let icons: BTreeSet<&String> = icon_by_extension.values().collect();
    let mut rendered_icons = Vec::new();
    for icon in &icons {
        let glyph = if *icon == GENERIC_ICON {
            None
        } else {
            let definition = theme
                .file_icons
                .get(*icon)
                .with_context(|| format!("icon {icon:?} is not in the theme"))?;
            let glyph_path = root.join("assets").join(&definition.path);
            Some(svg_body(&fs::read_to_string(&glyph_path)?)?)
        };
        rendered_icons.push((icon, render_ico(glyph.as_ref(), &logo)?));
    }

    let default_icon_pattern = Regex::new(
        r#"(?m)^Root: HKCU; Subkey: "Software\\Classes\\\{#RegValueName\}\.([^\\"]+)\\DefaultIcon";.*$"#,
    )?;
    let mut without_association = Vec::new();
    let mut with_default_icon = BTreeSet::new();
    let installer = default_icon_pattern.replace_all(&installer, |captures: &regex::Captures| {
        let extension = &captures[1];
        let Some(icon) = icon_by_extension.get(extension) else {
            without_association.push(extension.to_string());
            return captures[0].to_string();
        };
        with_default_icon.insert(extension.to_string());
        format!(
            r#"Root: HKCU; Subkey: "Software\Classes\{{#RegValueName}}.{extension}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{INSTALLED_DIR}\{icon}.ico"; Tasks: associatewithfiles"#
        )
    });
    if !without_association.is_empty() {
        return Err(anyhow!(
            "DefaultIcon entries without a file association: {without_association:?}"
        ));
    }
    let without_default_icon: Vec<&String> = icon_by_extension
        .keys()
        .filter(|extension| !with_default_icon.contains(*extension))
        .collect();
    if !without_default_icon.is_empty() {
        return Err(anyhow!(
            "file associations without a DefaultIcon entry: {without_default_icon:?}"
        ));
    }

    let output_dir = root.join(OUTPUT_DIR);
    fs::create_dir_all(&output_dir)?;
    for entry in fs::read_dir(&output_dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "ico") {
            fs::remove_file(&path)?;
        }
    }
    for (icon, ico) in rendered_icons {
        fs::write(output_dir.join(format!("{icon}.ico")), ico)?;
    }
    fs::write(&installer_path, installer.as_ref())?;

    println!(
        "Wrote {} icons for {} file types to {OUTPUT_DIR}",
        icons.len(),
        icon_by_extension.len()
    );
    Ok(())
}

fn icon_for_extension(extension: &str, theme: &IconTheme) -> String {
    let lowercase = extension.to_lowercase();
    let generic_path = theme.file_icons.get(GENERIC_ICON).map(|icon| &icon.path);
    EXTRA_ICONS
        .iter()
        .find(|(extra, _)| *extra == lowercase)
        .map(|(_, icon)| icon.to_string())
        .or_else(|| theme.file_suffixes.get(&lowercase).cloned())
        .or_else(|| theme.file_stems.get(&format!(".{lowercase}")).cloned())
        .or_else(|| theme.file_stems.get(&lowercase).cloned())
        .filter(|icon| {
            theme
                .file_icons
                .get(icon)
                .is_some_and(|definition| Some(&definition.path) != generic_path)
        })
        .unwrap_or_else(|| GENERIC_ICON.to_string())
}

struct SvgBody {
    view_box: String,
    attributes: String,
    content: String,
}

fn svg_body(svg: &str) -> Result<SvgBody> {
    let open_tag = Regex::new(r"(?s)<svg\b([^>]*)>")?;
    let captures = open_tag.captures(svg).context("SVG has no <svg> element")?;
    let attributes = &captures[1];
    let attribute = |name: &str| -> Option<String> {
        Regex::new(&format!(r#"(?:^|\s){name}="([^"]*)""#))
            .ok()?
            .captures(attributes)
            .map(|captures| captures[1].to_string())
    };
    let view_box = match (
        attribute("viewBox"),
        attribute("width"),
        attribute("height"),
    ) {
        (Some(view_box), _, _) => view_box,
        (None, Some(width), Some(height)) => format!("0 0 {width} {height}"),
        _ => return Err(anyhow!("SVG has no viewBox or size")),
    };
    let placement = Regex::new(r#"(^|\s)(x|y|width|height|viewBox)="[^"]*""#)?;
    let start = captures.get(0).map_or(0, |open| open.end());
    let end = svg.rfind("</svg>").context("SVG has no closing tag")?;
    Ok(SvgBody {
        view_box,
        attributes: placement.replace_all(attributes, "$1").trim().to_string(),
        content: svg[start..end].to_string(),
    })
}

fn render_ico(glyph: Option<&SvgBody>, logo: &SvgBody) -> Result<Vec<u8>> {
    let mut images = Vec::new();
    for size in SIZES {
        let svg = document_svg(glyph, logo, size);
        let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())?;
        let mut pixmap =
            tiny_skia::Pixmap::new(size, size).context("failed to allocate an icon image")?;
        let scale = size as f32 / 256.;
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        images.push((size, pixmap.encode_png()?));
    }
    Ok(encode_ico(&images))
}

fn document_svg(glyph: Option<&SvgBody>, logo: &SvgBody, size: u32) -> String {
    let with_badge = size >= SMALLEST_SIZE_WITH_BADGE;
    let stroke = (256. / size as f32).max(6.);
    let (glyph_x, glyph_y, glyph_size) = if with_badge {
        (60, 56, 104)
    } else {
        (56, 76, 144)
    };
    let glyph = glyph
        .map(|glyph| {
            format!(
                r#"<svg x="{glyph_x}" y="{glyph_y}" width="{glyph_size}" height="{glyph_size}" viewBox="{}" {}>{}</svg>"#,
                glyph.view_box, glyph.attributes, glyph.content
            )
        })
        .unwrap_or_default();
    let badge = if with_badge {
        format!(
            r#"<svg x="144" y="164" width="80" height="80" viewBox="{}" {}>{}</svg>"#,
            logo.view_box, logo.attributes, logo.content
        )
    } else {
        String::new()
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">
<path d="M52 12H164L220 68V232Q220 244 208 244H52Q40 244 40 232V24Q40 12 52 12Z" fill="#FFFFFF" stroke="#B8BEC7" stroke-width="{stroke}" stroke-linejoin="round"/>
<path d="M164 12V56Q164 68 176 68H220" fill="#E4E7EB" stroke="#B8BEC7" stroke-width="{stroke}" stroke-linejoin="round"/>
{glyph}
{badge}
</svg>"##
    )
}

fn encode_ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let header_size = 6 + 16 * images.len();
    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = header_size;
    for (size, png) in images {
        let dimension = if *size >= 256 { 0 } else { *size as u8 };
        ico.extend_from_slice(&[dimension, dimension, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in images {
        ico.extend_from_slice(png);
    }
    ico
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ico_directory_points_at_each_image() {
        let images = vec![(16, vec![1, 2, 3]), (256, vec![4, 5])];
        let ico = encode_ico(&images);
        assert_eq!(&ico[0..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!(ico[6], 16);
        assert_eq!(ico[22], 0);
        let first_offset = u32::from_le_bytes(ico[18..22].try_into().unwrap()) as usize;
        let second_offset = u32::from_le_bytes(ico[34..38].try_into().unwrap()) as usize;
        assert_eq!(&ico[first_offset..first_offset + 3], &[1, 2, 3]);
        assert_eq!(&ico[second_offset..second_offset + 2], &[4, 5]);
    }

    #[test]
    fn svg_body_keeps_attributes_that_only_end_in_a_size_name() {
        let body =
            svg_body(r#"<svg stroke-width="2" width="24" height="20"><path/></svg>"#).unwrap();
        assert_eq!(body.view_box, "0 0 24 20");
        assert_eq!(body.attributes, r#"stroke-width="2""#);
    }

    #[test]
    fn svg_body_keeps_root_styling_and_falls_back_to_the_size_for_the_view_box() {
        let body =
            svg_body(r#"<svg width="24" height="20" fill="none"><path d="M0 0"/></svg>"#).unwrap();
        assert_eq!(body.view_box, "0 0 24 20");
        assert_eq!(body.attributes, r#"fill="none""#);
        assert_eq!(body.content, r#"<path d="M0 0"/>"#);
    }
}
