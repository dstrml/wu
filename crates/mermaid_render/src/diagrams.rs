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

struct Requested {
    diagram: Diagram,
    request: u64,
}

#[derive(Default)]
pub struct MermaidDiagrams {
    theme: Option<DiagramTheme>,
    generation: usize,
    requests: u64,
    diagrams: HashMap<String, Requested>,
    dropped: HashSet<String>,
    tasks: HashMap<String, Task<()>>,
}

impl MermaidDiagrams {
    pub fn get(&self, source: &str) -> Option<&Diagram> {
        self.diagrams
            .get(&normalize(source))
            .map(|requested| &requested.diagram)
    }

    /// Whether `source` was dropped to make room and has to be requested again to show.
    pub fn was_dropped(&self, source: &str) -> bool {
        !self.dropped.is_empty() && self.dropped.contains(&normalize(source))
    }

    pub fn is_ready(&self, source: &str) -> bool {
        matches!(self.get(source), Some(Diagram::Ready { .. }))
    }

    /// Changes whenever drawn diagrams are dropped, so callers know to request them again.
    pub fn generation(&self) -> usize {
        self.generation
    }

    /// Keeps at most `MAX_DIAGRAMS`, dropping the ones requested longest ago.
    pub fn request(&mut self, source: &str, theme: &DiagramTheme, cx: &mut Context<Self>) {
        self.use_theme(theme);
        let key = normalize(source);
        self.requests += 1;
        if let Some(requested) = self.diagrams.get_mut(&key) {
            requested.request = self.requests;
            return;
        }
        if self.diagrams.len() >= MAX_DIAGRAMS {
            self.drop_oldest();
        }
        self.dropped.remove(&key);
        self.draw(key, theme, cx);
    }

    /// Keeps exactly the listed diagrams, however many, for views that show one document.
    pub fn show_only(&mut self, sources: &[&str], theme: &DiagramTheme, cx: &mut Context<Self>) {
        self.use_theme(theme);
        self.retain(sources);
        for source in sources {
            let key = normalize(source);
            if !self.diagrams.contains_key(&key) {
                self.draw(key, theme, cx);
            }
        }
    }

    fn retain(&mut self, sources: &[&str]) {
        let sources = sources
            .iter()
            .map(|source| normalize(source))
            .collect::<HashSet<_>>();
        self.diagrams.retain(|source, _| sources.contains(source));
        self.tasks.retain(|source, _| sources.contains(source));
    }

    fn use_theme(&mut self, theme: &DiagramTheme) {
        if self.theme.as_ref() != Some(theme) {
            self.theme = Some(theme.clone());
            self.diagrams.clear();
            self.tasks.clear();
            self.dropped.clear();
            self.generation += 1;
        }
    }

    fn draw(&mut self, key: String, theme: &DiagramTheme, cx: &mut Context<Self>) {
        self.diagrams.insert(
            key.clone(),
            Requested {
                diagram: Diagram::Pending,
                request: self.requests,
            },
        );
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
                    if let Some(requested) = this.diagrams.get_mut(&key) {
                        requested.diagram = diagram;
                    }
                    cx.notify();
                })
                .log_err();
            }
        });
        self.tasks.insert(key, task);
    }

    fn drop_oldest(&mut self) {
        let oldest = self
            .diagrams
            .iter()
            .min_by_key(|(_, requested)| requested.request)
            .map(|(source, _)| source.clone());
        if let Some(oldest) = oldest {
            self.diagrams.remove(&oldest);
            self.tasks.remove(&oldest);
            self.dropped.insert(oldest);
            self.generation += 1;
        }
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

        diagrams.update(cx, |diagrams, cx| {
            diagrams.show_only(&[flowchart], &theme(0.), cx)
        });
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

    #[gpui::test]
    fn only_the_oldest_diagram_makes_room_and_one_document_keeps_them_all(cx: &mut TestAppContext) {
        let diagrams = cx.new(|_| MermaidDiagrams::default());
        let source = |index: usize| format!("flowchart TD\nA --> B{index}\n");
        diagrams.update(cx, |diagrams, cx| {
            for index in 0..MAX_DIAGRAMS {
                diagrams.request(&source(index), &theme(0.), cx);
            }
            diagrams.request(&source(0), &theme(0.), cx);
        });
        cx.run_until_parked();
        let generation = diagrams.read_with(cx, |diagrams, _| diagrams.generation());

        diagrams.update(cx, |diagrams, cx| {
            diagrams.request(&source(MAX_DIAGRAMS), &theme(0.), cx)
        });
        cx.run_until_parked();
        diagrams.read_with(cx, |diagrams, _| {
            assert!(diagrams.get(&source(1)).is_none());
            assert!(diagrams.was_dropped(&source(1)));
            assert!(
                diagrams.is_ready(&source(0)),
                "asked for again, so it stays"
            );
            assert!(diagrams.is_ready(&source(2)));
            assert!(diagrams.is_ready(&source(MAX_DIAGRAMS)));
            assert_ne!(diagrams.generation(), generation);
        });

        diagrams.update(cx, |diagrams, cx| {
            diagrams.request(&source(1), &theme(0.), cx)
        });
        cx.run_until_parked();
        diagrams.read_with(cx, |diagrams, _| {
            assert!(diagrams.is_ready(&source(1)));
            assert!(!diagrams.was_dropped(&source(1)));
            assert!(diagrams.was_dropped(&source(2)));
        });

        let document: Vec<String> = (0..MAX_DIAGRAMS + 6).map(source).collect();
        let document: Vec<&str> = document.iter().map(String::as_str).collect();
        diagrams.update(cx, |diagrams, cx| {
            diagrams.show_only(&document, &theme(0.), cx)
        });
        cx.run_until_parked();
        diagrams.read_with(cx, |diagrams, _| {
            assert!(document.iter().all(|source| diagrams.is_ready(source)));
        });
    }
}
