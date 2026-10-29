use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct Meta {
    pub auth_base_url: String,
    pub linkit_base_url: String,
    pub cybion_base_url: String,
    pub public_base_url: String,
}

#[derive(Serialize)]
pub struct Me {
    pub id: String,
    pub display_name: Option<String>,
    pub cybion_connected: bool,
    pub cybion_key_hint: Option<String>,
    pub cybion_last_error: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_user_id: String,
    pub owner_name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub open_tickets: i64,
    pub total_tickets: i64,
    pub resolver_ready: bool,
    pub resolver_thread_url: Option<String>,
    /// The Cybion Ultimate Machine that watches this project's queue.
    pub resolver_machine_id: Option<String>,
    pub is_owner: bool,
    /// `public` or `private`. Private projects are visible only to the owner
    /// and the users listed in `project_grants`.
    pub visibility: String,
    /// True when the viewer is on the project's grant list (the owner gets
    /// `is_owner` instead).
    pub is_member: bool,
}

#[derive(Serialize)]
pub struct TicketSummary {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub number: i64,
    pub title: String,
    pub status: String,
    pub reporter_user_id: String,
    pub reporter_name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub comment_count: i64,
    pub has_evidence: bool,
    pub thread_id: Option<String>,
}

#[derive(Serialize)]
pub struct CommentView {
    pub id: String,
    pub kind: String,
    pub author_kind: String,
    pub author_user_id: Option<String>,
    pub author_name: Option<String>,
    pub body: String,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct ThreadView {
    /// The Cybion Thread bound to this Ticket, when one exists.
    pub thread_id: Option<String>,
    pub owner_user_id: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub display_status: Option<String>,
    pub context_tokens: Option<i64>,
    /// Only the Thread's owner may open it in Cybion.
    pub url: Option<String>,
    /// Set when creating or probing the Thread failed.
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct AgentEndpoint {
    pub comments_url: String,
    pub ticket_url: String,
    pub close_url: String,
}

#[derive(Serialize)]
pub struct TicketView {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub status: String,
    pub reporter_user_id: String,
    pub reporter_name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub submitted_at: Option<i64>,
    pub closed_at: Option<i64>,
    pub closed_by_kind: Option<String>,
    pub closed_by_name: Option<String>,
    pub is_reporter: bool,
    pub is_project_owner: bool,
    pub comments: Vec<CommentView>,
    pub reporter_thread: ThreadView,
    pub resolver_thread_url: Option<String>,
    pub agent: AgentEndpoint,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProject {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Missing means private: a new project is nobody else's business until its
    /// owner says otherwise.
    #[serde(default)]
    pub visibility: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateProject {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub resolver_thread_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateGrant {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct GrantView {
    pub user_id: String,
    pub created_at: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTicket {
    pub title: String,
    #[serde(default)]
    pub body: String,
    /// `true` submits straight away; `false` keeps the Ticket as a draft so the
    /// reporter Agent can help fill it in first.
    #[serde(default)]
    pub submit: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateTicket {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateComment {
    pub body: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectCybion {
    /// Optional manual key. The UI never asks for one: Tickets mints the key
    /// through Cybion's browser API instead.
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentComment {
    pub body: String,
    /// `comment` (default) or `evidence` (the reporter Agent's findings).
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentClose {
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Serialize)]
pub struct QueueTicket {
    pub id: String,
    pub number: i64,
    pub title: String,
    pub project: String,
    pub reporter: String,
    pub url: String,
    pub updated_at: i64,
    pub comments: i64,
    pub summary: String,
    pub evidence: Option<String>,
}

#[derive(Serialize)]
pub struct QueueView {
    pub project: String,
    pub total_open: i64,
    pub pending: Vec<QueueTicket>,
}

/// Collapses whitespace and truncates to `limit` characters on a char boundary.
pub fn excerpt(text: &str, limit: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= limit {
        return collapsed;
    }
    let cut: String = collapsed.chars().take(limit).collect();
    format!("{cut}…")
}

pub fn is_agent(kind: &str) -> bool {
    matches!(kind, "reporter_agent" | "resolver_agent")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_collapses_whitespace_and_keeps_short_text() {
        assert_eq!(excerpt("  磁盘\n\n剩余 3% \t", 40), "磁盘 剩余 3%");
        assert_eq!(excerpt("", 10), "");
    }

    #[test]
    fn excerpt_truncates_on_a_char_boundary() {
        let text = "故".repeat(10);
        let cut = excerpt(&text, 4);
        assert_eq!(cut, "故故故故…");
        assert_eq!(cut.chars().count(), 5);
    }

    #[test]
    fn agent_author_kinds_are_exactly_the_two_agents() {
        assert!(is_agent("reporter_agent"));
        assert!(is_agent("resolver_agent"));
        for kind in ["user", "system", ""] {
            assert!(!is_agent(kind), "{kind}");
        }
    }
}
