import { AppLayout } from "@zccz14/ux"
import { BotIcon, FolderIcon, SettingsIcon, TicketCheckIcon } from "lucide-react"
import { type ReactNode, useCallback, useState } from "react"

import { PageTitleContext } from "@/lib/page-title"
import { useCopy } from "@/lib/session"
import { TicketsMark } from "./mark"

export function AppShell({ children }: { children: ReactNode }) {
  const { t } = useCopy()
  const [pageTitle, setPageTitle] = useState<string | undefined>(undefined)
  const update = useCallback((next: string | undefined) => setPageTitle(next), [])
  return (
    <AppLayout
      pageTitle={pageTitle}
      logo={<TicketsMark />}
      title={t("appName")}
      nav={[
        {
          label: t("navWorkbench"),
          items: [
            { to: "/projects", label: t("navProjects"), icon: <FolderIcon /> },
            { to: "/my", label: t("navMyTickets"), icon: <TicketCheckIcon /> },
          ],
        },
        {
          label: t("navAgentGroup"),
          items: [{ to: "/guide", label: t("navAgent"), icon: <BotIcon /> }],
        },
        {
          label: t("navAccount"),
          items: [{ to: "/settings", label: t("navSettings"), icon: <SettingsIcon /> }],
        },
      ]}
    >
      <PageTitleContext.Provider value={update}>{children}</PageTitleContext.Provider>
    </AppLayout>
  )
}
