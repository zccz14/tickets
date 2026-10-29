export type ApiFn = <T>(path: string, init?: RequestInit) => Promise<T>

export type Meta = {
  auth_base_url: string
  linkit_base_url: string
  cybion_base_url: string
  public_base_url: string
}

export type Me = {
  id: string
  display_name: string | null
  cybion_connected: boolean
  cybion_key_hint: string | null
  cybion_last_error: string | null
}

export type ProjectVisibility = "public" | "private"

export type Project = {
  id: string
  name: string
  description: string
  owner_user_id: string
  owner_name: string | null
  created_at: number
  updated_at: number
  open_tickets: number
  total_tickets: number
  resolver_ready: boolean
  resolver_thread_url: string | null
  resolver_machine_id: string | null
  is_owner: boolean
  visibility: ProjectVisibility
  is_member: boolean
}

export type ProjectGrant = {
  user_id: string
  created_at: number
}

export type CybionWorker = {
  id: string
  label: string
  status: string
  last_seen_at: number | null
}

export type Machine = {
  id: string
  name: string
  worker_id: string
  worker_label: string | null
  interval_seconds: number
  thread_id: string
  thread_title: string | null
  enabled: boolean
  last_run_at: number | null
  last_exit: number | null
  created_at: number
}

export type TicketStatus = "draft" | "open" | "closed"

export type TicketSummary = {
  id: string
  project_id: string
  project_name: string
  number: number
  title: string
  status: TicketStatus
  reporter_user_id: string
  reporter_name: string | null
  created_at: number
  updated_at: number
  comment_count: number
  has_evidence: boolean
  thread_id: string | null
}

export type Comment = {
  id: string
  kind: "comment" | "evidence" | "system"
  author_kind: "user" | "reporter_agent" | "resolver_agent" | "system"
  author_user_id: string | null
  author_name: string | null
  body: string
  created_at: number
}

export type ThreadInfo = {
  thread_id: string | null
  owner_user_id: string | null
  title: string | null
  status: string | null
  display_status: string | null
  context_tokens: number | null
  url: string | null
  error: string | null
}

export type Ticket = {
  id: string
  project_id: string
  project_name: string
  number: number
  title: string
  body: string
  status: TicketStatus
  reporter_user_id: string
  reporter_name: string | null
  created_at: number
  updated_at: number
  submitted_at: number | null
  closed_at: number | null
  closed_by_kind: string | null
  closed_by_name: string | null
  is_reporter: boolean
  is_project_owner: boolean
  comments: Comment[]
  reporter_thread: ThreadInfo
  resolver_thread_url: string | null
  agent: { comments_url: string; ticket_url: string; close_url: string }
}

export type ResolverKey = { token: string; hint: string }

/** Builds the fetch wrapper: bearer token, JSON errors, one retry on 401. */
export function createRequest(
  token: () => Promise<string>,
  extraHeaders?: () => Record<string, string>,
): ApiFn {
  return async function request<T>(path: string, init?: RequestInit): Promise<T> {
    const send = async (bearer: string) =>
      fetch(path, {
        ...init,
        headers: {
          Authorization: `Bearer ${bearer}`,
          "Content-Type": "application/json",
          ...(extraHeaders?.() ?? {}),
          ...(init?.headers ?? {}),
        },
      })
    let response = await send(await token())
    if (response.status === 401) response = await send(await token())
    if (!response.ok) {
      const body = (await response.json().catch(() => ({ error: response.statusText }))) as {
        error?: string
      }
      throw Object.assign(new Error(body.error ?? response.statusText), { status: response.status })
    }
    if (response.status === 204) return undefined as T
    return (await response.json()) as T
  }
}
