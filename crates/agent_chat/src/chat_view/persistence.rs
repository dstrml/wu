use super::ChatView;
use crate::{AgentChatSettings, session::AgentStore};
use anyhow::{Context as _, Result, anyhow};
use db::{query, sqlez::domain::Domain, sqlez_macros::sql};
use gpui::{App, AppContext as _, Context, Entity, Task, WeakEntity, Window};
use project::Project;
use settings::Settings as _;
use workspace::{
    ItemId, Workspace, WorkspaceDb, WorkspaceId, delete_unloaded_items, item::SerializableItem,
};

pub struct ChatTabsDb(db::sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for ChatTabsDb {
    const NAME: &str = stringify!(ChatTabsDb);

    const MIGRATIONS: &[&str] = &[sql!(
        CREATE TABLE agent_chat_tabs (
            workspace_id INTEGER,
            item_id INTEGER UNIQUE,
            chat_id TEXT NOT NULL,

            PRIMARY KEY(workspace_id, item_id),
            FOREIGN KEY(workspace_id) REFERENCES workspaces(workspace_id)
            ON DELETE CASCADE
        ) STRICT;
    )];
}

db::static_connection!(ChatTabsDb, [WorkspaceDb]);

impl ChatTabsDb {
    query! {
        pub async fn save_chat_tab(
            item_id: ItemId,
            workspace_id: WorkspaceId,
            chat_id: String
        ) -> Result<()> {
            INSERT OR REPLACE INTO agent_chat_tabs(item_id, workspace_id, chat_id)
            VALUES (?, ?, ?)
        }
    }

    query! {
        pub fn get_chat_tab(item_id: ItemId, workspace_id: WorkspaceId) -> Result<Option<String>> {
            SELECT chat_id
            FROM agent_chat_tabs
            WHERE item_id = ? AND workspace_id = ?
        }
    }
}

impl SerializableItem for ChatView {
    fn serialized_item_kind() -> &'static str {
        "AgentChat"
    }

    fn cleanup(
        workspace_id: WorkspaceId,
        alive_items: Vec<ItemId>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let db = ChatTabsDb::global(cx);
        delete_unloaded_items(alive_items, workspace_id, "agent_chat_tabs", &db, cx)
    }

    fn deserialize(
        project: Entity<Project>,
        workspace: WeakEntity<Workspace>,
        workspace_id: WorkspaceId,
        item_id: ItemId,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Entity<Self>>> {
        if !AgentChatSettings::get_global(cx).enabled {
            return Task::ready(Err(anyhow!("agent chat is turned off")));
        }
        let db = ChatTabsDb::global(cx);
        let store = AgentStore::global(project.read(cx).languages().clone(), cx);
        window.spawn(cx, async move |cx| {
            let chat_id = db
                .get_chat_tab(item_id, workspace_id)?
                .context("no chat to restore")?;
            let session = store
                .update(cx, |store, cx| store.open_saved_session(chat_id.into(), cx))
                .await?;
            cx.update(|window, cx| {
                cx.new(|cx| {
                    let mut view = ChatView::new(session, store, project, workspace, window, cx);
                    view.update_check_due = true;
                    view
                })
            })
        })
    }

    fn serialize(
        &mut self,
        workspace: &mut Workspace,
        item_id: ItemId,
        _closing: bool,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<()>>> {
        if self.is_subagent() {
            return None;
        }
        let workspace_id = workspace.database_id()?;
        let chat_id = self.session.read(cx).metadata().id.to_string();
        let db = ChatTabsDb::global(cx);
        Some(cx.background_spawn(
            async move { db.save_chat_tab(item_id, workspace_id, chat_id).await },
        ))
    }

    fn should_serialize(&self, _event: &Self::Event) -> bool {
        false
    }
}
