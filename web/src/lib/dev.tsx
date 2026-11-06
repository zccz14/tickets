import { useCallback, useMemo, type ReactNode } from "react"

import { createRequest, type Meta } from "./api"
import { SessionContext } from "./session"

/**
 * Local-only session: the backend runs with `--dev-user`, so every request is
 * authenticated as one fixed user and no Auth Mini round-trip is needed.
 *
 * Guarded by `import.meta.env.DEV`, which Vite folds to `false` in a build, so
 * a production bundle cannot reach this path.
 */
export function DevSessionProvider({ meta, children }: { meta: Meta; children: ReactNode }) {
  // `--dev-user` means the backend answers as one fixed user, so the literal
  // token below is the one the browser hands to Cybion in local development.
  const getToken = useCallback(async () => "dev", [])
  const request = useMemo(
    () => createRequest(getToken, () => ({ "X-Linkit-Username": "dev" })),
    [getToken],
  )
  const value = useMemo(
    () => ({ request, lang: "zh" as const, getToken, cybionBaseUrl: meta.cybion_base_url }),
    [request, getToken, meta.cybion_base_url],
  )
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>
}
