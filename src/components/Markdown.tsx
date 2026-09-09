import { Fragment, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

// A deliberately small Markdown renderer for LLM-produced research text:
// headings, bold / italic / inline code, links, and bullet / numbered lists.
// Not a full CommonMark implementation — just what the model actually emits.

const INLINE =
  /(\*\*([^*]+)\*\*)|(__([^_]+)__)|(\*([^*\n]+)\*)|(_([^_\n]+)_)|(`([^`]+)`)|(\[([^\]]+)\]\((https?:\/\/[^)\s]+)\))/g;

export function MarkdownInline({ text }: { text: string }): ReactNode {
  const out: ReactNode[] = [];
  let last = 0;
  let key = 0;
  let m: RegExpExecArray | null;
  INLINE.lastIndex = 0;
  while ((m = INLINE.exec(text)) !== null) {
    if (m.index > last) out.push(<Fragment key={key++}>{text.slice(last, m.index)}</Fragment>);
    if (m[2] || m[4]) {
      out.push(<strong key={key++}>{m[2] ?? m[4]}</strong>);
    } else if (m[6] || m[8]) {
      out.push(<em key={key++}>{m[6] ?? m[8]}</em>);
    } else if (m[10]) {
      out.push(
        <code
          key={key++}
          className="rounded bg-black/5 px-1 py-0.5 text-[0.85em] dark:bg-white/10"
        >
          {m[10]}
        </code>,
      );
    } else if (m[12]) {
      const href = m[13];
      out.push(
        <button
          key={key++}
          onClick={() => openUrl(href)}
          className="text-[var(--accent)] underline"
        >
          {m[12]}
        </button>,
      );
    }
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(<Fragment key={key++}>{text.slice(last)}</Fragment>);
  return out;
}

const BLOCK_LEAD = /^(#{1,4}\s|\s*[-*+]\s|\s*\d+\.\s|\s*>\s?)/;

export default function Markdown({ text }: { text: string }) {
  const lines = (text ?? "").replace(/\r\n/g, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let i = 0;
  let key = 0;

  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }

    const heading = line.match(/^(#{1,4})\s+(.*)$/);
    if (heading) {
      const level = heading[1].length;
      blocks.push(
        <p
          key={key++}
          className={
            level <= 2
              ? "mt-3 text-sm font-semibold"
              : "mt-2 text-xs font-semibold uppercase tracking-wide text-[var(--muted)]"
          }
        >
          <MarkdownInline text={heading[2]} />
        </p>,
      );
      i++;
      continue;
    }

    if (/^\s*[-*+]\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\s*[-*+]\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*[-*+]\s+/, ""));
        i++;
      }
      blocks.push(
        <ul key={key++} className="list-disc space-y-1 pl-5">
          {items.map((it, j) => (
            <li key={j}>
              <MarkdownInline text={it} />
            </li>
          ))}
        </ul>,
      );
      continue;
    }

    if (/^\s*\d+\.\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*\d+\.\s+/, ""));
        i++;
      }
      blocks.push(
        <ol key={key++} className="list-decimal space-y-1 pl-5">
          {items.map((it, j) => (
            <li key={j}>
              <MarkdownInline text={it} />
            </li>
          ))}
        </ol>,
      );
      continue;
    }

    if (/^\s*>\s?/.test(line)) {
      const quote: string[] = [];
      while (i < lines.length && /^\s*>\s?/.test(lines[i])) {
        quote.push(lines[i].replace(/^\s*>\s?/, ""));
        i++;
      }
      blocks.push(
        <blockquote
          key={key++}
          className="border-l-2 border-[var(--border)] pl-3 text-[var(--muted)]"
        >
          <MarkdownInline text={quote.join(" ")} />
        </blockquote>,
      );
      continue;
    }

    const para: string[] = [];
    while (i < lines.length && lines[i].trim() && !BLOCK_LEAD.test(lines[i])) {
      para.push(lines[i].trim());
      i++;
    }
    blocks.push(
      <p key={key++}>
        <MarkdownInline text={para.join(" ")} />
      </p>,
    );
  }

  return <div className="space-y-2 text-sm leading-relaxed">{blocks}</div>;
}
