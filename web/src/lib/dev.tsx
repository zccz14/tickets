import { useMemo, type ReactNode } from "react"

import { createRequest } from "./api"
import { SessionContext } from "./session"

/**
 * Local-only session: the backend runs with `--dev-user`, so every request is
 * authenticated as one fixed user and no Auth Mini round-trip is needed.
 *
 * Guarded by `import.meta.env.DEV`, which Vite folds to `false` in a build, so
 * a production bundle cannot reach this path.
 */
export function DevSessionProvider({ children }: { children: ReactNode }) {
  const request = useMemo(
    () => createRequest(async () => "dev", () => ({ "X-Linkit-Username": "dev" })),
    [],
  )
  const value = useMemo(() => ({ request, lang: "zh" as const }), [request])
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>
}
