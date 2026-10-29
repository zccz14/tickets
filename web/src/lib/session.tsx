import { createContext, useContext, useMemo, type ReactNode } from "react"
import { useAuthMini } from "auth-mini-react-components"
import { useLinkit } from "linkit-react-components"
import type { AuthMiniApi } from "auth-mini/sdk/browser"

import { createRequest, type ApiFn } from "./api"
import { resolveLang, translate, type CopyKey, type Lang } from "./i18n"

type SessionValue = {
  request: ApiFn
  lang: Lang
}

export const SessionContext = createContext<SessionValue | null>(null)

async function accessToken(sdk: AuthMiniApi): Promise<string> {
  const session = sdk.session.getState()
  if (!session.accessToken) throw new Error("Auth Mini session is not available")
  const expiresAt = Date.parse(session.expiresAt ?? "")
  if (!Number.isFinite(expiresAt) || expiresAt <= Date.now() + 30_000) {
    const refreshed = await sdk.session.refresh()
    if (!refreshed.accessToken) throw new Error("Auth Mini session refresh failed")
    return refreshed.accessToken
  }
  return session.accessToken
}

/** Auth Mini token + Linkit identity, in one place for every page. */
export function SessionProvider({ children }: { children: ReactNode }) {
  const { sdk } = useAuthMini()
  const linkit = useLinkit()
  const lang = resolveLang(linkit.lang)
  const request = useMemo(
    () =>
      createRequest(
        async () => {
          if (!sdk) throw new Error("Auth Mini 尚未就绪")
          return accessToken(sdk)
        },
        (): Record<string, string> => {
          const username = linkit.myProfile?.username
          if (!username) return {}
          return { "X-Linkit-Username": username }
        },
      ),
    [sdk, linkit.myProfile?.username],
  )
  const value = useMemo(() => ({ request, lang }), [request, lang])
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>
}

export function useSession(): SessionValue {
  const value = useContext(SessionContext)
  if (!value) throw new Error("useSession must be used inside SessionProvider")
  return value
}

export function useCopy() {
  const { lang } = useSession()
  return useMemo(
    () => ({
      lang,
      t: (key: CopyKey, vars?: Record<string, string | number>) => translate(lang, key, vars),
    }),
    [lang],
  )
}
