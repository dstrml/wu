use super::{
    ChatView, DOWNLOADED_FEEDBACK, GENERATED_IMAGE_MAX_HEIGHT, GENERATED_IMAGE_MAX_WIDTH, ui,
};
use crate::chat_style::ink;
use gpui::{
    AnyElement, Context, Image, ImageSource, IntoElement, MouseButton, ObjectFit, ParentElement,
    Pixels, ScrollWheelEvent, SharedString, Styled, Window, deferred, div, hsla, img, px, relative,
};
use i18n::t;
use std::{
    fs::{File, OpenOptions},
    io::{ErrorKind, Write as _},
    path::{Path, PathBuf},
    sync::Arc,
};
use theme::ActiveTheme as _;
use ui::{Icon, IconName, IconSize, Tooltip, prelude::*};
use util::ResultExt as _;
use workspace::{Toast, notifications::NotificationId};

const MIN_ZOOM: f32 = 1.;
const MAX_ZOOM: f32 = 6.;
const SCRIM_ALPHA: f32 = 0.7;
const SCRIM_ALPHA_DARK_DEFAULT: f32 = 0.6;

pub(super) struct Lightbox {
    pub source: ImageSource,
    pub name: SharedString,
    pub zoom: f32,
    pub image: ChatImage,
}

#[derive(Clone)]
pub(super) enum ChatImage {
    File(PathBuf),
    Data(Arc<Image>),
}

fn downloads_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| paths::home_dir().join("Downloads"))
}

/// Claims the name on creation, so two downloads at once never share a file.
fn create_unused(directory: &Path, file_name: &str) -> std::io::Result<(PathBuf, File)> {
    let name = Path::new(file_name);
    let stem = name
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = name
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    let candidates = std::iter::once(directory.join(file_name))
        .chain((2..).map(|number| directory.join(format!("{stem} ({number}){extension}"))));
    for path in candidates {
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(ErrorKind::AlreadyExists.into())
}

impl ChatImage {
    pub(super) fn download_key(&self) -> SharedString {
        match self {
            ChatImage::File(path) => format!("download-{}", path.display()).into(),
            ChatImage::Data(image) => format!("download-{}", image.id()).into(),
        }
    }

    fn file_name(&self) -> String {
        match self {
            ChatImage::File(path) => path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "image".into()),
            ChatImage::Data(_) => "diagram.svg".into(),
        }
    }
}

impl ChatView {
    pub(super) fn open_image(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let name: SharedString = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into();
        self.lightbox = Some(Lightbox {
            source: path.clone().into(),
            name,
            zoom: 1.,
            image: ChatImage::File(path),
        });
        window.focus(&self.lightbox_focus, cx);
        cx.notify();
    }

    pub(super) fn generated_image_layout(&self, path: &Path) -> Option<(Pixels, f32)> {
        *self
            .generated_image_layouts
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let (width, height) = image::image_dimensions(path).log_err()?;
                if width == 0 || height == 0 {
                    return None;
                }
                let (width, height) = (width as f32, height as f32);
                let scale = (GENERATED_IMAGE_MAX_WIDTH / width)
                    .min(GENERATED_IMAGE_MAX_HEIGHT / height)
                    .min(1.);
                Some((px(width * scale), width / height))
            })
    }

    pub(super) fn open_image_data(
        &mut self,
        image: Arc<Image>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.lightbox = Some(Lightbox {
            source: image.clone().into(),
            name: t!("Mermaid diagram"),
            zoom: 1.,
            image: ChatImage::Data(image),
        });
        window.focus(&self.lightbox_focus, cx);
        cx.notify();
    }

    pub(super) fn download_image(
        &mut self,
        image: ChatImage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_image_in(downloads_dir(), image, window, cx);
    }

    fn save_image_in(
        &mut self,
        directory: PathBuf,
        image: ChatImage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let file_name = image.file_name();
        let key = image.download_key();
        let workspace = self.workspace.clone();
        let saved = cx.background_spawn(async move {
            std::fs::create_dir_all(&directory)?;
            let (destination, mut file) = create_unused(&directory, &file_name)?;
            let written = match image {
                ChatImage::File(path) => {
                    File::open(path).and_then(|mut source| std::io::copy(&mut source, &mut file))
                }
                ChatImage::Data(image) => file.write_all(&image.bytes).map(|()| 0),
            };
            if written.is_err() {
                std::fs::remove_file(&destination).log_err();
            }
            written.map(|_| ())
        });
        cx.spawn_in(window, async move |this, cx| match saved.await {
            Ok(()) => this
                .update(cx, |this, cx| this.show_done(key, DOWNLOADED_FEEDBACK, cx))
                .log_err(),
            Err(error) => {
                let message = t!("Couldn't save the image: {error}", error = error).to_string();
                workspace
                    .update(cx, |workspace, cx| {
                        workspace.show_toast(
                            Toast::new(NotificationId::unique::<ChatView>(), message).autohide(),
                            cx,
                        )
                    })
                    .log_err()
            }
        })
        .detach();
    }

    pub(super) fn close_image(&mut self, cx: &mut Context<Self>) {
        if self.lightbox.take().is_some() {
            cx.notify();
        }
    }

    fn zoom_image(&mut self, factor: f32, cx: &mut Context<Self>) {
        if let Some(lightbox) = &mut self.lightbox {
            lightbox.zoom = (lightbox.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
            cx.notify();
        }
    }

    pub(super) fn render_lightbox(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let lightbox = self.lightbox.as_ref()?;
        let zoom = lightbox.zoom;
        let scrim_alpha = if cx.theme().appearance.is_light() {
            0.32 * (SCRIM_ALPHA / SCRIM_ALPHA_DARK_DEFAULT)
        } else {
            SCRIM_ALPHA
        };
        let button = |id: &'static str, icon: IconName| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(hsla(0., 0., 0., 0.55))
                .cursor_pointer()
                .hover(|style| style.bg(hsla(0., 0., 0., 0.75)))
                .child(Icon::new(icon).size(IconSize::Small).color(Color::Custom(gpui::white())))
        };
        Some(
            deferred(
                div()
                    .id("agent-lightbox")
                    .track_focus(&self.lightbox_focus)
                    .key_context("AgentLightbox")
                    .on_action(cx.listener(|this, _: &menu::Cancel, _, cx| this.close_image(cx)))
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(hsla(0., 0., 0., scrim_alpha))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.close_image(cx)),
                    )
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                        if !event.modifiers.platform && !event.modifiers.control {
                            return;
                        }
                        let delta = event.delta.pixel_delta(window.line_height()).y;
                        this.zoom_image(if f32::from(delta) > 0. { 1.1 } else { 1. / 1.1 }, cx);
                    }))
                    .child(
                        div()
                            .id("agent-lightbox-scroll")
                            .size_full()
                            .overflow_scroll()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(12.))
                            .child(
                                div()
                                    .w(relative(0.9 * zoom))
                                    .h(relative(0.85 * zoom))
                                    .flex_none()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .child(
                                        img(lightbox.source.clone())
                                            .size_full()
                                            .object_fit(if zoom > MIN_ZOOM {
                                                ObjectFit::Contain
                                            } else {
                                                ObjectFit::ScaleDown
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .max_w(relative(0.9))
                                    .flex_none()
                                    .overflow_hidden()
                                    .text_size(ui(11.))
                                    .text_color(ink(0.45, cx))
                                    .child(lightbox.name.clone()),
                            ),
                    )
                    .child(
                        h_flex()
                            .absolute()
                            .top(px(14.))
                            .right(px(14.))
                            .gap(px(8.))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                div()
                                    .px(px(10.))
                                    .py(px(5.))
                                    .rounded_full()
                                    .bg(hsla(0., 0., 0., 0.55))
                                    .text_size(ui(12.))
                                    .text_color(gpui::white())
                                    .child(format!("{:.0}%", zoom * 100.)),
                            )
                            .child({
                                let image = lightbox.image.clone();
                                let downloaded =
                                    self.copied.as_ref() == Some(&image.download_key());
                                button(
                                    "agent-lightbox-download",
                                    if downloaded {
                                        IconName::Check
                                    } else {
                                        IconName::Download
                                    },
                                )
                                .tooltip(Tooltip::text(if downloaded {
                                    t!("Saved to Downloads")
                                } else {
                                    t!("Download")
                                }))
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.download_image(image.clone(), window, cx)
                                    },
                                ))
                            })
                            .child(
                                button("agent-lightbox-zoom-out", IconName::Dash).on_click(
                                    cx.listener(|this, _, _, cx| this.zoom_image(1. / 1.25, cx)),
                                ),
                            )
                            .child(
                                button("agent-lightbox-zoom-in", IconName::Plus).on_click(
                                    cx.listener(|this, _, _, cx| this.zoom_image(1.25, cx)),
                                ),
                            )
                            .child(
                                button("agent-lightbox-close", IconName::AgentClose)
                                    .on_click(cx.listener(|this, _, _, cx| this.close_image(cx))),
                            ),
                    ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentKind,
        session::tests::{new_store, run_until},
    };
    use gpui::{TestAppContext, WeakEntity};
    use project::FakeFs;

    #[gpui::test]
    async fn downloads_never_replace_an_existing_file(cx: &mut TestAppContext) {
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
        });
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let project = project::Project::test(FakeFs::new(cx.executor()), [], cx).await;
        let session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, directory.path().to_path_buf(), cx)
        });
        let source = directory.path().join("generated.png");
        std::fs::write(&source, b"png bytes").expect("write image");
        let downloads = directory.path().join("Downloads");
        let (view, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(
                session.clone(),
                store.clone(),
                project,
                WeakEntity::new_invalid(),
                window,
                cx,
            )
        });
        let image = ChatImage::File(source);
        view.update_in(cx, |view, window, cx| {
            for _ in 0..2 {
                view.save_image_in(downloads.clone(), image.clone(), window, cx)
            }
        });
        let saved = |name: &str| std::fs::read(downloads.join(name)).ok();
        run_until(cx, |_| {
            saved("generated.png").as_deref() == Some(b"png bytes")
                && saved("generated (2).png").as_deref() == Some(b"png bytes")
        });
        assert!(saved("generated (3).png").is_none());
        let shown = view.read_with(cx, |view, _| view.copied.clone());
        assert_eq!(shown, Some(image.download_key()));
    }
}
