//! SchemaSidebar — the left tree of tables, views, indexes and triggers.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::schema_sidebar_view::schema_sidebar_view;
use crate::database::database_constant::SCHEMA_GROUPS;
use crate::database::database_structure::{SchemaObject, TabKind};

// Keval Thumar
/// Click a table/view to browse it, an index/trigger (or the ⓘ button) to see
/// its structure.
#[component]
pub fn SchemaSidebar(#[prop(into)] schema: Signal<Vec<SchemaObject>>, on_open: Callback<TabKind>) -> impl IntoView {
    let filter = RwSignal::new(String::new());
    let collapsed = RwSignal::new(Vec::<&'static str>::new());

    // Objects per group, filtered by name.
    let groups = Memo::new(move |_| {
        let needle = filter.get().to_lowercase();
        SCHEMA_GROUPS
            .iter()
            .map(|(kind, _)| {
                let items: Vec<SchemaObject> = schema.with(|s| {
                    s.iter().filter(|o| o.kind == *kind && o.name.to_lowercase().contains(&needle)).cloned().collect()
                });
                (*kind, items)
            })
            .collect::<Vec<_>>()
    });

    let on_toggle = Callback::new(move |kind: &'static str| {
        collapsed.update(|c| match c.iter().position(|k| *k == kind) {
            Some(i) => {
                c.remove(i);
            }
            None => c.push(kind),
        })
    });
    let on_item = Callback::new(move |o: SchemaObject| {
        on_open.run(if o.is_browsable() { TabKind::Browse(o.name) } else { TabKind::Structure(o.name) })
    });
    let on_structure = Callback::new(move |name: String| on_open.run(TabKind::Structure(name)));

    schema_sidebar_view(filter, groups, collapsed.into(), on_toggle, on_item, on_structure)
}
