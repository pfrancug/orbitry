const FG: Record<number, string> = {
  30: "#1f2937",
  31: "#f87171",
  32: "#34d399",
  33: "#fbbf24",
  34: "#60a5fa",
  35: "#e879f9",
  36: "#22d3ee",
  37: "#e5e7eb",
  90: "#6b7280",
  91: "#fca5a5",
  92: "#6ee7b7",
  93: "#fde68a",
  94: "#93c5fd",
  95: "#f0abfc",
  96: "#67e8f9",
  97: "#ffffff",
};

const BG: Record<number, string> = {
  40: "#111827",
  41: "#7f1d1d",
  42: "#064e3b",
  43: "#78350f",
  44: "#1e3a8a",
  45: "#6b21a8",
  46: "#155e75",
  47: "#e5e7eb",
  100: "#374151",
  101: "#991b1b",
  102: "#047857",
  103: "#b45309",
  104: "#1d4ed8",
  105: "#7e22ce",
  106: "#0e7490",
  107: "#f9fafb",
};

type Style = {
  bold: boolean;
  dim: boolean;
  underline: boolean;
  fg: string;
  bg: string;
};

function escapeHtml(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function openSpan(style: Style): string {
  const rules: string[] = [];
  if (style.bold) rules.push("font-weight:700");
  if (style.dim) rules.push("opacity:0.7");
  if (style.underline) rules.push("text-decoration:underline");
  if (style.fg) rules.push(`color:${style.fg}`);
  if (style.bg) rules.push(`background:${style.bg}`);
  return rules.length ? `<span style="${rules.join(";")}">` : "";
}

/** Convert a log line with ANSI SGR codes into safe HTML. */
export function ansiToHtml(input: string): string {
  const cleaned = input
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b\[[0-9;?]*[A-HJKSTfhlm]/g, (match) => (match.endsWith("m") ? match : ""));

  const style: Style = { bold: false, dim: false, underline: false, fg: "", bg: "" };
  let html = "";
  let open = "";
  const re = /\x1b\[([0-9;]*)m/g;
  let last = 0;
  let match: RegExpExecArray | null;

  const flush = (from: number, to: number) => {
    const chunk = escapeHtml(cleaned.slice(from, to));
    if (!chunk) return;
    if (open) html += open + chunk + "</span>";
    else html += chunk;
  };

  while ((match = re.exec(cleaned))) {
    flush(last, match.index);
    last = match.index + match[0].length;
    const codes = match[1] === "" ? [0] : match[1].split(";").map((part) => Number(part) || 0);
    for (const code of codes) {
      if (code === 0) {
        style.bold = false;
        style.dim = false;
        style.underline = false;
        style.fg = "";
        style.bg = "";
      } else if (code === 1) style.bold = true;
      else if (code === 2) style.dim = true;
      else if (code === 4) style.underline = true;
      else if (code === 22) {
        style.bold = false;
        style.dim = false;
      } else if (code === 24) style.underline = false;
      else if (code === 39) style.fg = "";
      else if (code === 49) style.bg = "";
      else if (FG[code]) style.fg = FG[code];
      else if (BG[code]) style.bg = BG[code];
    }
    open = openSpan(style);
  }
  flush(last, cleaned.length);
  return html;
}
