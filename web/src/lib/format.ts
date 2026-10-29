import { translate, type Lang } from "./i18n.ts"

/** Unix seconds → "3 小时前", falling back to a date past a week. */
export function relativeTime(lang: Lang, seconds: number): string {
  const delta = Math.max(0, Math.floor(Date.now() / 1000) - seconds)
  if (delta < 60) return translate(lang, "timeJustNow")
  if (delta < 3600) return translate(lang, "timeMinutesAgo", { n: Math.floor(delta / 60) })
  if (delta < 86_400) return translate(lang, "timeHoursAgo", { n: Math.floor(delta / 3600) })
  if (delta < 7 * 86_400) return translate(lang, "timeDaysAgo", { n: Math.floor(delta / 86_400) })
  return new Date(seconds * 1000).toLocaleDateString(lang === "zh" ? "zh-CN" : "en-US")
}

export function absoluteTime(lang: Lang, seconds: number): string {
  return new Date(seconds * 1000).toLocaleString(lang === "zh" ? "zh-CN" : "en-US")
}

export function shortId(id: string, length = 8): string {
  return id.slice(0, length)
}
