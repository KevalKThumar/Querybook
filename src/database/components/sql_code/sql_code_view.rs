//! Template for highlighted SQL.
//!
//! Author: Keval Thumar

use leptos::html::Pre;
use leptos::prelude::*;

pub fn sql_code_view(tokens: Memo<Vec<(&'static str, String)>>, class: String, pre_ref: NodeRef<Pre>) -> impl IntoView {
    view! {
        <pre class=class node_ref=pre_ref>
            {move || {
                tokens.get().into_iter().map(|(class, text)| view! { <span class=class>{text}</span> }).collect_view()
            }}
        </pre>
    }
}
