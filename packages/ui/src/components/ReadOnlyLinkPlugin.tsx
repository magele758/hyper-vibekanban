import { useEffect } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import { LinkNode } from '@lexical/link';

/**
 * Sanitize href to block dangerous protocols.
 * Returns undefined if the href is blocked.
 */
function sanitizeHref(href?: string): string | undefined {
  if (typeof href !== 'string') return undefined;
  const trimmed = href.trim();
  // Block dangerous protocols
  if (/^(javascript|vbscript|data):/i.test(trimmed)) return undefined;
  // Allow anchors and common relative forms (but they'll be disabled)
  if (
    trimmed.startsWith('#') ||
    trimmed.startsWith('./') ||
    trimmed.startsWith('../') ||
    trimmed.startsWith('/')
  )
    return trimmed;
  // Allow only https
  if (/^https:\/\//i.test(trimmed)) return trimmed;
  // Block everything else by default
  return undefined;
}

/**
 * Check if href is an external HTTPS link.
 */
function isExternalHref(href?: string): boolean {
  if (!href) return false;
  return /^https:\/\//i.test(href);
}

/**
 * If the href points to a local markdown file (relative or root-relative,
 * not an external URL), return the cleaned repo-relative path. Otherwise null.
 *
 * Accepts forms like `docs/foo.md`, `./foo.md`, `/docs/foo.md`, and strips any
 * `#anchor` or `?query` suffix. External `https://` links are excluded.
 */
function markdownFilePath(href?: string): string | null {
  if (typeof href !== 'string') return null;
  const trimmed = href.trim();
  if (!trimmed) return null;
  // Only local/relative links — skip external and protocol-relative URLs.
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(trimmed) || trimmed.startsWith('//')) {
    return null;
  }
  // Strip query and fragment.
  const withoutFragment = trimmed.split('#')[0].split('?')[0];
  if (!/\.(md|markdown)$/i.test(withoutFragment)) return null;
  // Normalize to a repo-relative path (drop leading ./ and /).
  const cleaned = withoutFragment.replace(/^\.\//, '').replace(/^\/+/, '');
  return cleaned || null;
}

interface ReadOnlyLinkPluginProps {
  /**
   * When provided, relative links to markdown files stay clickable and invoke
   * this callback with the repo-relative path instead of being disabled.
   */
  onMarkdownFileClick?: (path: string) => void;
}

/**
 * Plugin that handles link sanitization and security attributes in read-only mode.
 * - Blocks dangerous protocols (javascript:, vbscript:, data:)
 * - External HTTPS links: clickable with target="_blank" and rel="noopener noreferrer"
 * - Relative links to `.md`/`.markdown` files: clickable, open in the preview panel
 *   (when `onMarkdownFileClick` is provided)
 * - Other internal/relative links: rendered but not clickable
 */
export function ReadOnlyLinkPlugin({
  onMarkdownFileClick,
}: ReadOnlyLinkPluginProps = {}) {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    // Apply sanitization + click behavior to a single anchor element.
    const applyLinkBehavior = (link: HTMLAnchorElement) => {
      const href = link.getAttribute('href');
      const safeHref = sanitizeHref(href ?? undefined);

      if (!safeHref) {
        // Dangerous protocol - remove href entirely
        link.removeAttribute('href');
        link.style.cursor = 'not-allowed';
        link.style.pointerEvents = 'none';
        return;
      }

      if (isExternalHref(safeHref)) {
        // External HTTPS link - add security attributes
        link.setAttribute('target', '_blank');
        link.setAttribute('rel', 'noopener noreferrer');
        link.onclick = (e) => e.stopPropagation();
        return;
      }

      // Relative markdown file link - keep clickable, open in preview panel
      const mdPath = onMarkdownFileClick ? markdownFilePath(href ?? undefined) : null;
      if (mdPath && onMarkdownFileClick) {
        link.removeAttribute('href');
        link.style.cursor = 'pointer';
        link.style.pointerEvents = 'auto';
        link.setAttribute('role', 'link');
        link.title = href ?? '';
        link.onclick = (e) => {
          e.preventDefault();
          e.stopPropagation();
          onMarkdownFileClick(mdPath);
        };
        return;
      }

      // Other internal/relative link - disable clicking
      link.removeAttribute('href');
      link.style.cursor = 'not-allowed';
      link.style.pointerEvents = 'none';
      link.setAttribute('role', 'link');
      link.setAttribute('aria-disabled', 'true');
      link.title = href ?? '';
    };

    // Register a mutation listener to modify link DOM elements
    const unregister = editor.registerMutationListener(
      LinkNode,
      (mutations) => {
        for (const [nodeKey, mutation] of mutations) {
          if (mutation === 'destroyed') continue;

          const dom = editor.getElementByKey(nodeKey);
          if (!dom || !(dom instanceof HTMLAnchorElement)) continue;

          applyLinkBehavior(dom);
        }
      }
    );

    // Also handle existing links on mount by triggering a read
    editor.getEditorState().read(() => {
      const root = editor.getRootElement();
      if (!root) return;
      root.querySelectorAll('a').forEach(applyLinkBehavior);
    });

    return unregister;
  }, [editor, onMarkdownFileClick]);

  return null;
}
