use db::kvp::KeyValueStore;
use dev_container::find_configs_in_snapshot;
use gpui::{App, SharedString, Window};
use i18n::t;
use project::{Project, WorktreeId};
use std::path::Path;
use std::sync::LazyLock;
use ui::Tooltip;
use ui::prelude::*;
use util::ResultExt;
use util::rel_path::RelPath;
use workspace::Workspace;
use workspace::notifications::NotificationId;
use workspace::notifications::simple_message_notification::MessageNotification;
use worktree::UpdatedEntriesSet;

const DEV_CONTAINER_SUGGEST_KEY: &str = "dev_container_suggest_dismissed";

fn devcontainer_dir_path() -> &'static RelPath {
    static PATH: LazyLock<&'static RelPath> =
        LazyLock::new(|| RelPath::from_unix_str(".devcontainer").expect("valid path"));
    *PATH
}

fn devcontainer_json_path() -> &'static RelPath {
    static PATH: LazyLock<&'static RelPath> =
        LazyLock::new(|| RelPath::from_unix_str(".devcontainer.json").expect("valid path"));
    *PATH
}

fn project_devcontainer_key(project_path: &str) -> String {
    format!("{}_{}", DEV_CONTAINER_SUGGEST_KEY, project_path)
}

// Keyed on the repo's common Git dir so dismissing in one git worktree covers its siblings.
fn dismiss_path_for_worktree(
    project: &gpui::Entity<Project>,
    worktree_abs_path: &Path,
    cx: &App,
) -> String {
    let common_dir = project
        .read(cx)
        .repositories(cx)
        .values()
        .filter_map(|repo| {
            let repo = repo.read(cx);
            let work_dir = repo.work_directory_abs_path.clone();
            // The folder may sit inside a repo or submodule; the innermost containing repo wins.
            worktree_abs_path
                .starts_with(work_dir.as_ref())
                .then(|| (work_dir.as_os_str().len(), repo.common_dir_abs_path.clone()))
        })
        .max_by_key(|(work_dir_len, _)| *work_dir_len)
        .map(|(_, common_dir)| common_dir);

    match common_dir {
        Some(common_dir) => common_dir.to_string_lossy().to_string(),
        None => worktree_abs_path.to_string_lossy().to_string(),
    }
}

pub fn suggest_on_worktree_updated(
    workspace: &mut Workspace,
    worktree_id: WorktreeId,
    updated_entries: &UpdatedEntriesSet,
    project: &gpui::Entity<Project>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if workspace.open_in_dev_container() {
        open_dev_container_from_cli(workspace, window, cx);
        return;
    }

    let devcontainer_updated = updated_entries.iter().any(|(path, _, _)| {
        path.as_ref() == devcontainer_dir_path() || path.as_ref() == devcontainer_json_path()
    });

    if !devcontainer_updated {
        return;
    }

    suggest_for_worktree(worktree_id, project, window, cx);
}

pub fn suggest_on_scans_complete(window: &mut Window, cx: &mut Context<Workspace>) {
    cx.spawn_in(window, async move |workspace, cx| {
        let scans_complete =
            workspace.update(cx, |workspace, cx| workspace.worktree_scans_complete(cx))?;
        scans_complete.await;
        workspace.update_in(cx, |workspace, window, cx| {
            if workspace.open_in_dev_container() {
                return;
            }
            let project = workspace.project().clone();
            let worktree_ids = project
                .read(cx)
                .visible_worktrees(cx)
                .map(|worktree| worktree.read(cx).id())
                .collect::<Vec<_>>();
            for worktree_id in worktree_ids {
                suggest_for_worktree(worktree_id, &project, window, cx);
            }
        })
    })
    .detach_and_log_err(cx);
}

fn suggest_for_worktree(
    worktree_id: WorktreeId,
    project: &gpui::Entity<Project>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(worktree) = project.read(cx).worktree_for_id(worktree_id, cx) else {
        return;
    };

    let worktree = worktree.read(cx);

    if !worktree.is_local() {
        return;
    }

    if find_configs_in_snapshot(worktree).is_empty() {
        return;
    }

    let abs_path = worktree.abs_path();
    let project_path = abs_path.to_string_lossy().to_string();
    let worktree_name = worktree.root_name_str().to_string();
    let dismiss_path = dismiss_path_for_worktree(project, abs_path.as_ref(), cx);
    let key_for_dismiss = project_devcontainer_key(&dismiss_path);

    let already_dismissed = KeyValueStore::global(cx)
        .read_kvp(&key_for_dismiss)
        .ok()
        .flatten()
        .is_some();

    if already_dismissed {
        return;
    }

    cx.on_next_frame(window, move |workspace, _window, cx| {
        struct DevContainerSuggestionNotification;

        let notification_id = NotificationId::composite::<DevContainerSuggestionNotification>(
            SharedString::from(project_path.clone()),
        );

        workspace.show_notification(notification_id, cx, |cx| {
            cx.new(move |cx| {
                let message: SharedString = t!(
                    "{worktree_name} contains a Dev Container configuration file. Would you like to re-open it in a container?",
                    worktree_name = worktree_name
                );
                let tooltip_text: SharedString = project_path.clone().into();
                MessageNotification::new_from_builder(cx, move |_window, _cx| {
                    div()
                        .id("dev-container-suggest-message")
                        .child(Label::new(message.clone()))
                        .tooltip(Tooltip::text(tooltip_text.clone()))
                        .into_any_element()
                })
                .primary_message(t!("Yes, Open in Container"))
                .primary_icon(IconName::Check)
                .primary_icon_color(Color::Success)
                .primary_on_click({
                    move |window, cx| {
                        window.dispatch_action(Box::new(wu_actions::OpenDevContainer), cx);
                    }
                })
                .secondary_message(t!("Don't Show Again"))
                .secondary_icon(IconName::Close)
                .secondary_icon_color(Color::Error)
                .secondary_on_click({
                    move |_window, cx| {
                        let key = key_for_dismiss.clone();
                        let kvp = KeyValueStore::global(cx);
                        cx.background_spawn(async move {
                            kvp.write_kvp(key, "dismissed".to_string())
                                .await
                                .log_err();
                        })
                        .detach();
                    }
                })
            })
        });
    });
}

pub fn open_dev_container_from_cli(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    workspace.set_open_in_dev_container(false);
    let task = cx.spawn_in(window, async move |workspace, cx| {
        let scans_complete =
            workspace.update(cx, |workspace, cx| workspace.worktree_scans_complete(cx))?;
        scans_complete.await;

        workspace.update_in(cx, |workspace, window, cx| {
            let has_configs = workspace
                .project()
                .read(cx)
                .worktrees(cx)
                .any(|worktree| !find_configs_in_snapshot(worktree.read(cx)).is_empty());
            if has_configs {
                cx.on_next_frame(window, move |_workspace, window, cx| {
                    window.dispatch_action(Box::new(wu_actions::OpenDevContainer), cx);
                });
            } else {
                log::warn!("--dev-container: no devcontainer configuration found in project");
            }
        })
    });
    workspace.set_dev_container_task(task);
}
