//! SqlCode — SQL text in a `<pre>` with syntax colours (shared lexer).
//! Used for CREATE statements in the structure view and as the SQL editor's
//! highlight layer (which passes `pre_ref` to sync scrolling).
//!
//! Author: Keval Thumar

use leptos::html::Pre;
use leptos::prelude::*;
use shared::lexer::{tokenize, Tok};

use super::sql_code_view::sql_code_view;

#[component]
pub fn SqlCode(
    #[prop(into)] sql: Signal<String>,
    /// Extra classes for the `<pre>`.
    #[prop(into, optional)]
    class: String,
    #[prop(optional)] pre_ref: NodeRef<Pre>,
) -> impl IntoView {
    let tokens = Memo::new(move |_| {
        sql.with(|s| {
            tokenize(s)
                .into_iter()
                .map(|(t, text)| {
                    let class = match t {
                        Tok::Keyword => "sqlcode-keyword",
                        Tok::Ident => "sqlcode-ident",
                        Tok::Str => "sqlcode-string",
                        Tok::Num => "sqlcode-number",
                        Tok::Comment => "sqlcode-comment",
                        Tok::Other => "",
                    };
                    (class, text.to_string())
                })
                .collect::<Vec<_>>()
        })
    });
    sql_code_view(tokens, format!("sqlcode {class}"), pre_ref)
}
