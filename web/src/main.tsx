import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query"
import { AuthMiniProvider } from "auth-mini-react-components"
import { LinkitProvider } from "linkit-react-components"
import { StrictMode, type ReactNode } from "react"
import { createRoot } from "react-dom/client"
import { HashRouter, Navigate, Route, Routes } from "react-router-dom"

import "./index.css"

import { AppShell } from "@/components/app-shell"
import type { Meta } from "@/lib/api"
import { DevSessionProvider } from "@/lib/dev"
import { SessionProvider } from "@/lib/session"
import { GuidePage } from "@/pages/guide"
import { MyTicketsPage } from "@/pages/my"
import { ProjectPage } from "@/pages/project"
import { ProjectsPage } from "@/pages/projects"
import { SettingsPage } from "@/pages/settings"
import { TicketPage } from "@/pages/ticket"

const client = new QueryClient({
  defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: false } },
})

/** Local development against `--dev-user`; always false in a build. */
const DEV_USER = import.meta.env.DEV ? import.meta.env.VITE_DEV_USER : undefined

async function loadMeta(): Promise<Meta> {
  const response = await fetch("/api/meta")
  if (!response.ok) throw new Error("无法读取服务配置")
  return (await response.json()) as Meta
}

function Pages() {
  return (
    <AppShell>
      <Routes>
        <Route path="/projects" element={<ProjectsPage />} />
        <Route path="/projects/:id" element={<ProjectPage />} />
        <Route path="/tickets/:id" element={<TicketPage />} />
        <Route path="/my" element={<MyTicketsPage />} />
        <Route path="/guide" element={<GuidePage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate replace to="/projects" />} />
      </Routes>
    </AppShell>
  )
}

function Session({ meta, children }: { meta: Meta; children: ReactNode }) {
  return (
    <AuthMiniProvider
      authMiniBaseUrl={meta.auth_base_url}
      audiences={[
        window.location.hostname,
        new URL(meta.linkit_base_url).hostname,
        new URL(meta.cybion_base_url).hostname,
      ]}
      autoRedirectToLogin={!DEV_USER}
    >
      <LinkitProvider linkitBaseUrl={meta.linkit_base_url} lang="zh-CN">
        {DEV_USER ? (
          <DevSessionProvider>{children}</DevSessionProvider>
        ) : (
          <SessionProvider>{children}</SessionProvider>
        )}
      </LinkitProvider>
    </AuthMiniProvider>
  )
}

function Root() {
  const meta = useQuery({ queryKey: ["meta"], queryFn: loadMeta, staleTime: Infinity, retry: 2 })
  if (meta.isError) {
    return (
      <div className="flex min-h-svh items-center justify-center p-8 text-sm text-muted-foreground">
        {(meta.error as Error).message}
      </div>
    )
  }
  if (!meta.data) {
    return (
      <div className="paper-wash flex min-h-svh items-center justify-center">
        <div className="size-6 animate-spin rounded-full border-2 border-primary/30 border-t-primary" />
      </div>
    )
  }
  return (
    <Session meta={meta.data}>
      <HashRouter>
        <Pages />
      </HashRouter>
    </Session>
  )
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={client}>
      <Root />
    </QueryClientProvider>
  </StrictMode>,
)
