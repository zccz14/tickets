import type { SVGProps } from "react"

/** The Tickets mark: a ticket stub with a perforation. Follows `currentColor`
 *  so the shell can mirror it into the favicon in either theme. */
export function TicketsMark(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <rect
        x="2.6"
        y="5.6"
        width="18.8"
        height="12.8"
        rx="3.2"
        stroke="currentColor"
        strokeWidth="1.7"
      />
      <path
        d="M14.6 6.1v11.8"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeDasharray="2.3 2.3"
      />
      <circle cx="7.6" cy="12" r="1.25" fill="currentColor" />
    </svg>
  )
}
