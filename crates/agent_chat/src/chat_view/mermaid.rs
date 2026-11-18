use super::ChatView;
use crate::chat_style::{accent, ink, page, text_faint};
use gpui::{App, Context};
use mermaid_render::DiagramTheme;
use std::ops::Range;
use theme::ActiveTheme as _;

pub(super) fn diagram_theme(cx: &App) -> DiagramTheme {
    let colors = cx.theme().colors();
    let is_light = cx.theme().appearance.is_light();
    let background = page(cx);
    let canvas = background.blend(ink(0.035, cx));
    let node = if is_light {
        background
    } else {
        canvas.blend(ink(0.06, cx))
    };
    let border_strong = ink(if is_light { 0.17 } else { 0.14 }, cx);
    DiagramTheme {
        font_family: "Inter".into(),
        background: canvas.into(),
        text: colors.text.into(),
        line: text_faint(cx).into(),
        node_fill: node.into(),
        node_border: border_strong.into(),
        accent: accent(cx).into(),
    }
}

pub(super) fn mermaid_sources(markdown: &str) -> Vec<String> {
    mermaid_blocks(markdown)
        .into_iter()
        .map(|(_, source)| source)
        .collect()
}

pub(super) fn mermaid_blocks(markdown: &str) -> Vec<(Range<usize>, String)> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for line in markdown.split_inclusive('\n') {
        lines.push((offset, line));
        offset += line.len();
    }
    let mut blocks = Vec::new();
    let mut lines = lines.into_iter();
    while let Some((start, line)) = lines.next() {
        let Some(info) = line.trim_start().strip_prefix("```") else {
            continue;
        };
        if !info.trim().eq_ignore_ascii_case("mermaid") {
            continue;
        }
        let mut source = String::new();
        for (body_start, body) in lines.by_ref() {
            if body.trim_start().starts_with("```") {
                blocks.push((start..body_start + body.len(), source));
                break;
            }
            source.push_str(body.trim_end_matches(['\n', '\r']));
            source.push('\n');
        }
    }
    blocks
}

impl ChatView {
    pub(super) fn request_visible_diagrams(&mut self, cx: &mut Context<Self>) {
        let theme = diagram_theme(cx);
        let generation = self.diagrams.read(cx).generation();
        if self.diagram_scan.0 != Some((theme.clone(), generation)) {
            self.diagram_scan.1.clear();
        }
        let mut scanned = Vec::new();
        let mut sources = Vec::new();
        for entry in self.entries(cx) {
            if let crate::session::Entry::Assistant { markdown, .. } = entry {
                let source = markdown.read(cx).source();
                let key = (markdown.entity_id(), source.len());
                if self.diagram_scan.1.get(&key.0) == Some(&key.1) {
                    continue;
                }
                scanned.push(key);
                if source.contains("```mermaid") {
                    sources.extend(mermaid_sources(source));
                }
            }
        }
        self.diagram_scan.1.extend(scanned);
        self.diagrams.update(cx, |diagrams, cx| {
            for source in &sources {
                diagrams.request(source, &theme, cx);
            }
        });
        let generation = self.diagrams.read(cx).generation();
        self.diagram_scan.0 = Some((theme, generation));
    }

    pub(super) fn drawn_diagram_ranges(&self, markdown: &str, cx: &App) -> Vec<Range<usize>> {
        if !markdown.contains("```") {
            return Vec::new();
        }
        let diagrams = self.diagrams.read(cx);
        mermaid_blocks(markdown)
            .into_iter()
            .filter(|(_, source)| {
                !self.diagram_sources_shown.contains(source.trim_end()) && diagrams.is_ready(source)
            })
            .map(|(range, _)| range)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::mermaid_sources;

    #[test]
    fn finds_finished_mermaid_fences_only() {
        let markdown = "Text\n```mermaid\ngraph TD\n  A-->B\n```\n```rust\nfn x() {}\n```\n";
        assert_eq!(mermaid_sources(markdown), ["graph TD\n  A-->B\n"]);
        assert!(mermaid_sources("```mermaid\ngraph TD\n  A-->").is_empty());
    }
}

#[cfg(test)]
mod block_tests {
    use super::*;

    #[test]
    fn mermaid_blocks_cover_the_whole_fence() {
        let markdown = "Intro\n```mermaid\ngraph TD\nA-->B\n```\nAfter\n```mermaid\nunclosed\n";
        let blocks = mermaid_blocks(markdown);
        assert_eq!(blocks.len(), 1);
        let (range, source) = &blocks[0];
        assert_eq!(&markdown[range.clone()], "```mermaid\ngraph TD\nA-->B\n```\n");
        assert_eq!(source, "graph TD\nA-->B\n");
        assert_eq!(mermaid_sources(markdown), vec![source.clone()]);
    }
}
