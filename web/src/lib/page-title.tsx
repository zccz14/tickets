import { createContext, useContext, useEffect } from "react"

export const PageTitleContext = createContext<(title: string | undefined) => void>(() => {})

/** Lets a detail page name itself in the shell header. */
export function usePageTitle(title: string | undefined) {
  const set = useContext(PageTitleContext)
  useEffect(() => {
    set(title)
    return () => set(undefined)
  }, [set, title])
}
