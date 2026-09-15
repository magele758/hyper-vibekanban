const REPLACEMENTS: Array<[string, string]> = [
  ['/favicon-vk-light.svg', '/favicon-maestro.svg'],
  ['/favicon-vk-dark.svg', '/favicon-maestro-dark.svg'],
  ['/apple-touch-icon.png', '/maestro-icon.png'],
  ['/site.webmanifest', '/site-maestro.webmanifest'],
  ['<title>Vibe Kanban</title>', '<title>Maestro</title>'],
];

export function applyMaestroLiteHtml(html: string): string {
  return REPLACEMENTS.reduce(
    (next, [from, to]) => next.replaceAll(from, to),
    html
  );
}
