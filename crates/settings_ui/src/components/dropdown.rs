use std::rc::Rc;

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString};
use heck::ToTitleCase as _;
use i18n::tc;
use ui::{
    ButtonSize, ContextMenu, DropdownMenu, DropdownStyle, FluentBuilder as _, IconPosition, px,
};

#[derive(IntoElement)]
pub struct EnumVariantDropdown {
    id: ElementId,
    selected_index: usize,
    labels: Rc<[SharedString]>,
    tab_index: Option<isize>,
    aria_label: Option<SharedString>,
    aria_description: Option<SharedString>,
    on_change: Rc<dyn Fn(usize, &mut ui::Window, &mut App) + 'static>,
}

impl EnumVariantDropdown {
    pub fn new<T>(
        id: impl Into<ElementId>,
        current_value: T,
        variants: &'static [T],
        labels: Rc<[SharedString]>,
        on_change: impl Fn(T, &mut ui::Window, &mut App) + 'static,
    ) -> Self
    where
        T: strum::VariantArray + strum::VariantNames + Copy + PartialEq + Send + Sync + 'static,
    {
        let selected_index = variants
            .iter()
            .position(|v| *v == current_value)
            .unwrap_or(0);
        Self::new_indexed(id, selected_index, labels, move |index, window, cx| {
            on_change(variants[index], window, cx)
        })
    }

    pub fn new_indexed(
        id: impl Into<ElementId>,
        selected_index: usize,
        labels: Rc<[SharedString]>,
        on_change: impl Fn(usize, &mut ui::Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            selected_index,
            labels,
            tab_index: None,
            aria_label: None,
            aria_description: None,
            on_change: Rc::new(on_change),
        }
    }

    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = Some(tab_index);
        self
    }

    /// Sets the label announced by assistive technology.
    /// Defaults to the currently selected value's label.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Sets the supplementary description announced by assistive technology
    /// after the combobox's name, role, and value (e.g. a setting subtitle).
    pub fn aria_description(mut self, description: impl Into<SharedString>) -> Self {
        self.aria_description = Some(description.into());
        self
    }
}

impl RenderOnce for EnumVariantDropdown {
    fn render(self, window: &mut ui::Window, cx: &mut ui::App) -> impl gpui::IntoElement {
        let current_value_label = self
            .labels
            .get(self.selected_index)
            .cloned()
            .unwrap_or_default();

        let context_menu = window.use_keyed_state(current_value_label.clone(), cx, |window, cx| {
            ContextMenu::new(window, cx, move |mut menu, _, _| {
                for (index, label) in self.labels.iter().enumerate() {
                    let on_change = self.on_change.clone();
                    menu = menu.toggleable_entry(
                        label.clone(),
                        index == self.selected_index,
                        IconPosition::End,
                        None,
                        move |window, cx| {
                            on_change(index, window, cx);
                        },
                    );
                }
                menu
            })
        });

        DropdownMenu::new(self.id, current_value_label, context_menu)
            .when_some(self.aria_label, |this, label| this.aria_label(label))
            .when_some(self.aria_description, |this, description| {
                this.aria_description(description)
            })
            .when_some(self.tab_index, |elem, tab_index| elem.tab_index(tab_index))
            .trigger_size(ButtonSize::Medium)
            .style(DropdownStyle::Outlined)
            .offset(gpui::Point {
                x: px(0.0),
                y: px(2.0),
            })
            .into_any_element()
    }
}

pub fn setting_value_label(label: &'static str, title_case: bool) -> SharedString {
    if i18n::language() != i18n::Language::English
        && let Some(translated) = translated_setting_value(label)
    {
        return translated;
    }
    if title_case {
        label.to_title_case().into()
    } else {
        SharedString::new_static(label)
    }
}

pub(crate) fn translated_setting_value(label: &str) -> Option<SharedString> {
    Some(match label {
        "Accept Edits" => tc!("setting value", "Accept Edits"),
        "ActiveEditor" => tc!("setting value", "Active Editor"),
        "Add to Existing Window" => tc!("setting value", "Add to Existing Window"),
        "AfterDelay" => tc!("setting value", "After Delay"),
        "All" => tc!("setting value", "All"),
        "AllEditors" => tc!("setting value", "All Editors"),
        "Alt" => tc!("setting value", "Alt"),
        "Always" => tc!("setting value", "Always"),
        "AlwaysHome" => tc!("setting value", "Always Home"),
        "Anywhere" => tc!("setting value", "Anywhere"),
        "Approve for Me" => tc!("setting value", "Approve for Me"),
        "Ask" => tc!("setting value", "Ask"),
        "Auto" => tc!("setting value", "Auto"),
        "Background" => tc!("setting value", "Background"),
        "Bar" => tc!("setting value", "Bar"),
        "Binary" => tc!("setting value", "Binary"),
        "Block" => tc!("setting value", "Block"),
        "Border" => tc!("setting value", "Border"),
        "Both" => tc!("setting value", "Both"),
        "Bottom" => tc!("setting value", "Bottom"),
        "Boundary" => tc!("setting value", "Boundary"),
        "Bounded" => tc!("setting value", "Bounded"),
        "Bypass" => tc!("setting value", "Bypass"),
        "Center" => tc!("setting value", "Center"),
        "Chevron" => tc!("setting value", "Chevron"),
        "Client" => tc!("setting value", "Client"),
        "CloseWindow" => tc!("setting value", "Close Window"),
        "CmdOrCtrl" => tc!("setting value", "Cmd or Ctrl"),
        "Combined" => tc!("setting value", "Combined"),
        "Comfortable" => tc!("setting value", "Comfortable"),
        "Compact" => tc!("setting value", "Compact"),
        "Contained" => tc!("setting value", "Contained"),
        "CurrentFileDirectory" => tc!("setting value", "Current File Directory"),
        "CurrentProjectDirectory" => tc!("setting value", "Current Project Directory"),
        "Custom" => tc!("setting value", "Custom"),
        "Dark" => tc!("setting value", "Dark"),
        "Decimal" => tc!("setting value", "Decimal"),
        "Default" => tc!("setting value", "Default"),
        "DefaultBranch" => tc!("setting value", "Default Branch"),
        "Detect" => tc!("setting value", "Detect"),
        "DirectoriesFirst" => tc!("setting value", "Directories First"),
        "Disabled" => tc!("setting value", "Disabled"),
        "Don't Ask" => tc!("setting value", "Don't Ask"),
        "Down" => tc!("setting value", "Down"),
        "Dynamic" => tc!("setting value", "Dynamic"),
        "EditorWidth" => tc!("setting value", "Editor Width"),
        "EmptyTab" => tc!("setting value", "Empty Tab"),
        "Enabled" => tc!("setting value", "Enabled"),
        "Enforce CRLF" => tc!("setting value", "Enforce CRLF"),
        "Enforce LF" => tc!("setting value", "Enforce LF"),
        "Enter" => tc!("setting value", "Enter"),
        "Error" => tc!("setting value", "Error"),
        "Errors" => tc!("setting value", "Errors"),
        "Expanded" => tc!("setting value", "Expanded"),
        "Fallback" => tc!("setting value", "Fallback"),
        "FileDiff" => tc!("setting value", "File Diff"),
        "FileNameFirst" => tc!("setting value", "File Name First"),
        "FilePathFirst" => tc!("setting value", "File Path First"),
        "FilesFirst" => tc!("setting value", "Files First"),
        "FindAllReferences" => tc!("setting value", "Find All References"),
        "FirstProjectDirectory" => tc!("setting value", "First Project Directory"),
        "Fixed" => tc!("setting value", "Fixed"),
        "Full" => tc!("setting value", "Full"),
        "Full Access" => tc!("setting value", "Full Access"),
        "Grayscale" => tc!("setting value", "Grayscale"),
        "Gutter" => tc!("setting value", "Gutter"),
        "Head" => tc!("setting value", "HEAD"),
        "Hidden" => tc!("setting value", "Hidden"),
        "Hide" => tc!("setting value", "Hide"),
        "High" => tc!("setting value", "High"),
        "Hint" => tc!("setting value", "Hint"),
        "History" => tc!("setting value", "History"),
        "Hollow" => tc!("setting value", "Hollow"),
        "Hover" => tc!("setting value", "Hover"),
        "Icon" => tc!("setting value", "Icon"),
        "InComments" => tc!("setting value", "In Comments"),
        "InSelections" => tc!("setting value", "In Selections"),
        "IndentAware" => tc!("setting value", "Indent Aware"),
        "Indexed" => tc!("setting value", "Indexed"),
        "Info" => tc!("setting value", "Info"),
        "Information" => tc!("setting value", "Information"),
        "Inlay" => tc!("setting value", "Inlay"),
        "Inline" => tc!("setting value", "Inline"),
        "Insert" => tc!("setting value", "Insert"),
        "Instruction" => tc!("setting value", "Instruction"),
        "KeepWindowOpen" => tc!("setting value", "Keep Window Open"),
        "LabelColor" => tc!("setting value", "Label Color"),
        "Last Used" => tc!("setting value", "Last Used"),
        "LastSession" => tc!("setting value", "Last Session"),
        "LastWorkspace" => tc!("setting value", "Last Workspace"),
        "Launchpad" => tc!("setting value", "Launchpad"),
        "Left" => tc!("setting value", "Left"),
        "LeftAligned" => tc!("setting value", "Left Aligned"),
        "LeftNeighbour" => tc!("setting value", "Left Neighbour"),
        "LeftOnly" => tc!("setting value", "Left Only"),
        "LeftOpen" => tc!("setting value", "Left Open"),
        "Light" => tc!("setting value", "Light"),
        "Line" => tc!("setting value", "Line"),
        "Low" => tc!("setting value", "Low"),
        "Lower" => tc!("setting value", "Lower"),
        "Max" => tc!("setting value", "Max"),
        "Medium" => tc!("setting value", "Medium"),
        "Menu" => tc!("setting value", "Menu"),
        "Minimal" => tc!("setting value", "Minimal"),
        "Minimum" => tc!("setting value", "Minimum"),
        "Mixed" => tc!("setting value", "Mixed"),
        "Modifications" => tc!("setting value", "Modifications"),
        "ModificationsIfAvailable" => tc!("setting value", "Modifications If Available"),
        "ModifierEnter" => tc!("setting value", "Modifier + Enter"),
        "MultiBuffer" => tc!("setting value", "Multi Buffer"),
        "Name" => tc!("setting value", "Name"),
        "Native" => tc!("setting value", "Native"),
        "Neighbour" => tc!("setting value", "Neighbour"),
        "Never" => tc!("setting value", "Never"),
        "NonUtf8" => tc!("setting value", "Non-UTF-8"),
        "None" => tc!("setting value", "None"),
        "Off" => tc!("setting value", "Off"),
        "On" => tc!("setting value", "On"),
        "OnFocusChange" => tc!("setting value", "On Focus Change"),
        "OnTyping" => tc!("setting value", "On Typing"),
        "OnTypingAndAction" => tc!("setting value", "On Typing And Action"),
        "OnWindowChange" => tc!("setting value", "On Window Change"),
        "OnePage" => tc!("setting value", "One Page"),
        "Open" => tc!("setting value", "Open"),
        "Open a New Window" => tc!("setting value", "Open a New Window"),
        "Path" => tc!("setting value", "Path"),
        "Picker" => tc!("setting value", "Picker"),
        "PlatformDefault" => tc!("setting value", "Platform Default"),
        "Prefer CRLF" => tc!("setting value", "Prefer CRLF"),
        "Prefer LF" => tc!("setting value", "Prefer LF"),
        "PreferLine" => tc!("setting value", "Prefer Line"),
        "Preserve" => tc!("setting value", "Preserve"),
        "PreserveIndent" => tc!("setting value", "Preserve Indent"),
        "Program" => tc!("setting value", "Program"),
        "ProjectDiff" => tc!("setting value", "Project Diff"),
        "Queue" => tc!("setting value", "Queue"),
        "QuitApp" => tc!("setting value", "Quit App"),
        "Read Only" => tc!("setting value", "Read Only"),
        "Replace" => tc!("setting value", "Replace"),
        "ReplaceSubsequence" => tc!("setting value", "Replace Subsequence"),
        "ReplaceSuffix" => tc!("setting value", "Replace Suffix"),
        "Right" => tc!("setting value", "Right"),
        "RightAligned" => tc!("setting value", "Right Aligned"),
        "RightOpen" => tc!("setting value", "Right Open"),
        "Select" => tc!("setting value", "Select"),
        "Selection" => tc!("setting value", "Selection"),
        "Server" => tc!("setting value", "Server"),
        "Simple" => tc!("setting value", "Simple"),
        "Smart" => tc!("setting value", "Smart"),
        "Split" => tc!("setting value", "Split"),
        "StagedHollow" => tc!("setting value", "Staged Hollow"),
        "Staging" => tc!("setting value", "Staging"),
        "Standard" => tc!("setting value", "Standard"),
        "Statement" => tc!("setting value", "Statement"),
        "Static" => tc!("setting value", "Static"),
        "Status" => tc!("setting value", "Status"),
        "StatusBar" => tc!("setting value", "Status Bar"),
        "Steer" => tc!("setting value", "Steer"),
        "Subpixel" => tc!("setting value", "Subpixel"),
        "Symbol" => tc!("setting value", "Symbol"),
        "SyntaxAware" => tc!("setting value", "Syntax Aware"),
        "System" => tc!("setting value", "System"),
        "TerminalControlled" => tc!("setting value", "Terminal Controlled"),
        "Top" => tc!("setting value", "Top"),
        "TrackedFiles" => tc!("setting value", "Tracked Files"),
        "Trailing" => tc!("setting value", "Trailing"),
        "Underline" => tc!("setting value", "Underline"),
        "Unicode" => tc!("setting value", "Unicode"),
        "Unified" => tc!("setting value", "Unified"),
        "UnstagedHollow" => tc!("setting value", "Unstaged Hollow"),
        "Up" => tc!("setting value", "Up"),
        "Upper" => tc!("setting value", "Upper"),
        "VerticalScrollMargin" => tc!("setting value", "Vertical Scroll Margin"),
        "ViewFile" => tc!("setting value", "View File"),
        "Warning" => tc!("setting value", "Warning"),
        "WithArguments" => tc!("setting value", "With Arguments"),
        "Wrapped" => tc!("setting value", "Wrapped"),
        "X-High" => tc!("setting value", "X-High"),
        _ => return None,
    })
}
