use crate::{DiagramTheme, RenderError};
use collections::{HashMap, HashSet};
use gpui::{Context, Image, ImageFormat, Task};
use i18n::t;
use std::sync::Arc;
use util::ResultExt as _;

const MAX_DIAGRAMS: usize = 64;

#[derive(Clone)]
pub enum Diagram {
    Pending,
    Ready {
        image: Arc<Image>,
        width: f32,
        height: f32,
    },
    Failed(String),
}

#[derive(Default)]
pub struct MermaidDiagrams {
    theme: Option<DiagramTheme>,
    generation: usize,
    diagrams: HashMap<String, Diagram>,
    tasks: HashMap<String, Task<()>>,
}

impl MermaidDiagrams {
    pub fn get(&self, source: &str) -> Option<&Diagram> {
        self.diagrams.get(&normalize(source))
    }

    pub fn is_ready(&self, source: &str) -> bool {
        matches!(self.get(source), Some(Diagram::Ready { .. }))
    }

    /// Changes whenever drawn diagrams are dropped, so callers know to request them again.
    pub fn generation(&self) -> usize {
        self.generation
    }

    pub fn request(&mut self, source: &str, theme: &DiagramTheme, cx: &mut Context<Self>) {
        if self.theme.as_ref() != Some(theme) {
            self.theme = Some(theme.clone());
            self.clear();
        }
        let key = normalize(source);
        if self.diagrams.contains_key(&key) {
            return;
        }
        if self.diagrams.len() >= MAX_DIAGRAMS {
            self.clear();
        }
        self.diagrams.insert(key.clone(), Diagram::Pending);
        let source = key.clone();
        let theme = theme.clone();
        let task = cx.spawn({
            let key = key.clone();
            async move |this, cx| {
                let diagram = cx
                    .background_executor()
                    .spawn(async move { draw(&source, &theme) })
                    .await;
                this.update(cx, |this, cx| {
                    this.tasks.remove(&key);
                    this.diagrams.insert(key, diagram);
                    cx.notify();
                })
                .log_err();
            }
        });
        self.tasks.insert(key, task);
    }

    /// Drops every diagram whose source isn't listed, for views that show one document.
    pub fn retain(&mut self, sources: &[&str]) {
        let sources = sources
            .iter()
            .map(|source| normalize(source))
            .collect::<HashSet<_>>();
        self.diagrams.retain(|source, _| sources.contains(source));
        self.tasks.retain(|source, _| sources.contains(source));
    }

    fn clear(&mut self) {
        self.diagrams.clear();
        self.tasks.clear();
        self.generation += 1;
    }
}

/// Code blocks nested in lists keep their indentation, so strip what every line shares.
fn normalize(source: &str) -> String {
    let indent = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    source
        .trim_end()
        .lines()
        .map(|line| line.get(indent..).unwrap_or(line.trim_start()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn draw(source: &str, theme: &DiagramTheme) -> Diagram {
    match crate::render(source, theme) {
        Ok(rendered) => Diagram::Ready {
            image: Arc::new(Image::from_bytes(ImageFormat::Svg, rendered.svg.to_vec())),
            width: rendered.width,
            height: rendered.height,
        },
        Err(RenderError::TooComplex) => {
            Diagram::Failed(t!("This diagram is too large to draw.").to_string())
        }
        Err(error) => Diagram::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext as _, TestAppContext};

    fn theme(red: f32) -> DiagramTheme {
        let color = crate::Rgba {
            r: red,
            g: 0.5,
            b: 0.5,
            a: 1.0,
        };
        DiagramTheme {
            font_family: "Helvetica".into(),
            background: color,
            text: color,
            line: color,
            node_fill: color,
            node_border: color,
            accent: color,
        }
    }

    #[gpui::test]
    fn draws_requested_diagrams_and_forgets_dropped_ones(cx: &mut TestAppContext) {
        let diagrams = cx.new(|_| MermaidDiagrams::default());
        let flowchart = "flowchart TD\nA --> B\n";
        let broken = "this is not a diagram\n";
        diagrams.update(cx, |diagrams, cx| {
            diagrams.request(flowchart, &theme(0.), cx);
            diagrams.request(broken, &theme(0.), cx);
        });
        cx.run_until_parked();
        let generation = diagrams.read_with(cx, |diagrams, _| {
            assert!(diagrams.is_ready("  flowchart TD\n  A --> B"));
            assert!(matches!(diagrams.get(broken), Some(Diagram::Failed(_))));
            diagrams.generation()
        });

        diagrams.update(cx, |diagrams, _| diagrams.retain(&[flowchart]));
        diagrams.read_with(cx, |diagrams, _| {
            assert!(diagrams.get(broken).is_none());
            assert!(diagrams.is_ready(flowchart));
            assert_eq!(diagrams.generation(), generation);
        });

        diagrams.update(cx, |diagrams, cx| {
            diagrams.request(flowchart, &theme(1.), cx)
        });
        diagrams.read_with(cx, |diagrams, _| {
            assert!(matches!(diagrams.get(flowchart), Some(Diagram::Pending)));
            assert_ne!(diagrams.generation(), generation);
        });
    }
}
