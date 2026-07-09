import { useEffect, useRef, useState } from 'react';

// Serialize all mermaid operations to avoid concurrent render/initialize races
let mermaidQueue: Promise<void> = Promise.resolve();
let initializedTheme: string | null = null;
let parseErrorHandlerInstalled = false;

function configureMermaid(
  mermaid: typeof import('mermaid').default,
  theme: 'light' | 'dark'
) {
  const mermaidTheme = theme === 'dark' ? 'dark' : 'default';
  mermaid.initialize({
    startOnLoad: false,
    theme: mermaidTheme,
    securityLevel: 'strict',
    suppressErrorRendering: true,
  });
  if (!parseErrorHandlerInstalled) {
    mermaid.setParseErrorHandler(() => {});
    parseErrorHandlerInstalled = true;
  }
  initializedTheme = mermaidTheme;
}

export interface MermaidRenderResult {
  svg: string;
  error: string | null;
}

export function useMermaidRender(
  chart: string,
  theme: 'light' | 'dark'
): MermaidRenderResult {
  const [svg, setSvg] = useState('');
  const [error, setError] = useState<string | null>(null);
  const renderCountRef = useRef(0);

  useEffect(() => {
    let cancelled = false;
    const renderId = ++renderCountRef.current;

    mermaidQueue = mermaidQueue.then(async () => {
      if (cancelled) return;

      const elementId = `mermaid-${renderId}-${Date.now()}`;

      try {
        const { default: mermaid } = await import('mermaid');
        const mermaidTheme = theme === 'dark' ? 'dark' : 'default';

        if (initializedTheme !== mermaidTheme) {
          configureMermaid(mermaid, theme);
        }

        // Use renderId to ensure each render call gets a unique DOM element ID
        const { svg: renderedSvg } = await mermaid.render(elementId, chart);

        if (!cancelled) {
          setSvg(renderedSvg);
          setError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setError(
            err instanceof Error ? err.message : 'Failed to render diagram'
          );
          setSvg('');
        }
      } finally {
        if (!cancelled) {
          document.getElementById(`d${elementId}`)?.remove();
          document.getElementById(elementId)?.remove();
          document.getElementById(`i${elementId}`)?.remove();
        }
      }
    });

    return () => {
      cancelled = true;
    };
  }, [chart, theme]);

  return { svg, error };
}
