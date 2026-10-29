import { Fragment, type ReactNode } from "react"

/**
 * A deliberately small Markdown renderer: headings, lists, fenced code,
 * blockquotes, rules, and inline code/bold/links. Ticket bodies are written by
 * people and Agents on a support tracker, so this covers what they actually
 * write without pulling a parser (and its sanitizer) into the bundle.
 */
export function Markdown({ text }: { text: string }) {
  return <div className="prose-ticket text-sm">{renderBlocks(text)}</div>
}

function renderBlocks(text: string): ReactNode[] {
  const lines = text.replace(/\r\n/g, "\n").split("\n")
  const blocks: ReactNode[] = []
  let index = 0
  let key = 0

  while (index < lines.length) {
    const line = lines[index]

    if (line.trim().startsWith("```")) {
      const body: string[] = []
      index += 1
      while (index < lines.length && !lines[index].trim().startsWith("```")) {
        body.push(lines[index])
        index += 1
      }
      index += 1
      blocks.push(
        <pre key={key++}>
          <code>{body.join("\n")}</code>
        </pre>,
      )
      continue
    }

    if (line.trim() === "") {
      index += 1
      continue
    }

    const heading = /^(#{1,4})\s+(.*)$/.exec(line)
    if (heading) {
      const level = heading[1].length
      const content = inline(heading[2])
      blocks.push(
        level === 1 ? <h1 key={key++}>{content}</h1>
        : level === 2 ? <h2 key={key++}>{content}</h2>
        : <h3 key={key++}>{content}</h3>,
      )
      index += 1
      continue
    }

    if (/^(-{3,}|\*{3,})$/.test(line.trim())) {
      blocks.push(<hr key={key++} className="my-4 border-border" />)
      index += 1
      continue
    }

    if (line.trim().startsWith(">")) {
      const body: string[] = []
      while (index < lines.length && lines[index].trim().startsWith(">")) {
        body.push(lines[index].replace(/^\s*>\s?/, ""))
        index += 1
      }
      blocks.push(<blockquote key={key++}>{inline(body.join(" "))}</blockquote>)
      continue
    }

    if (/^\s*([-*+]|\d+\.)\s+/.test(line)) {
      const ordered = /^\s*\d+\./.test(line)
      const items: string[] = []
      while (index < lines.length && /^\s*([-*+]|\d+\.)\s+/.test(lines[index])) {
        items.push(lines[index].replace(/^\s*([-*+]|\d+\.)\s+/, ""))
        index += 1
      }
      const rendered = items.map((item, itemIndex) => (
        <li key={itemIndex}>{inline(item)}</li>
      ))
      blocks.push(ordered ? <ol key={key++}>{rendered}</ol> : <ul key={key++}>{rendered}</ul>)
      continue
    }

    const paragraph: string[] = []
    while (
      index < lines.length &&
      lines[index].trim() !== "" &&
      !lines[index].trim().startsWith("```") &&
      !/^(#{1,4})\s+/.test(lines[index]) &&
      !lines[index].trim().startsWith(">") &&
      !/^\s*([-*+]|\d+\.)\s+/.test(lines[index])
    ) {
      paragraph.push(lines[index])
      index += 1
    }
    blocks.push(<p key={key++}>{inline(paragraph.join(" "))}</p>)
  }

  return blocks
}

const INLINE = /(`[^`]+`|\*\*[^*]+\*\*|\[[^\]]+\]\([^)]+\))/g

function inline(text: string): ReactNode[] {
  const parts = text.split(INLINE).filter((part) => part !== "")
  return parts.map((part, index) => {
    if (part.startsWith("`") && part.endsWith("`") && part.length > 2) {
      return <code key={index}>{part.slice(1, -1)}</code>
    }
    if (part.startsWith("**") && part.endsWith("**") && part.length > 4) {
      return <strong key={index}>{part.slice(2, -2)}</strong>
    }
    const link = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(part)
    if (link) {
      const href = link[2]
      const safe = /^https?:\/\//.test(href)
      return (
        <Fragment key={index}>
          {safe ? (
            <a href={href} target="_blank" rel="noreferrer noopener">
              {link[1]}
            </a>
          ) : (
            link[1]
          )}
        </Fragment>
      )
    }
    return <Fragment key={index}>{part}</Fragment>
  })
}
