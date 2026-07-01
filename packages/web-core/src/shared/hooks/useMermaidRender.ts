import { useEffect, useRef, useState } from 'react';

// Serialize all mermaid operations to avoid concurrent render/initialize races
let mermaidQueue: Promise<void> = Promise.resolve();
let initializedTheme: string | null = null;

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

      try {
        const { default: mermaid } = await import('mermaid');
        const mermaidTheme = theme === 'dark' ? 'dark' : 'default';

        if (initializedTheme !== mermaidTheme) {
          mermaid.initialize({
            startOnLoad: false,
            theme: mermaidTheme,
            securityLevel: 'strict',
          });
          initializedTheme = mermaidTheme;
        }

        // Use renderId to ensure each render call gets a unique DOM element ID
        const { svg: renderedSvg } = await mermaid.render(
          `mermaid-${renderId}-${Date.now()}`,
          chart
        );

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
      }
    });

    return () => {
      cancelled = true;
    };
  }, [chart, theme]);

  return { svg, error };
}
