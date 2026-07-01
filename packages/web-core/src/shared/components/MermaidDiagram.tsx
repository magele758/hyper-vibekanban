import { useState } from 'react';
import { Maximize2 } from 'lucide-react';
import { useMermaidRender } from '@/shared/hooks/useMermaidRender';
import { MermaidPreviewDialog } from '@/shared/dialogs/shared/MermaidPreviewDialog';

interface MermaidDiagramProps {
  chart: string;
  theme: 'light' | 'dark';
}

export function MermaidDiagram({ chart, theme }: MermaidDiagramProps) {
  const { svg, error } = useMermaidRender(chart, theme);
  const [isHovered, setIsHovered] = useState(false);

  const openPreview = () => {
    void MermaidPreviewDialog.show({ chart, theme });
  };

  if (error) {
    return (
      <div className="rounded-sm border border-error/20 bg-error/5 p-base">
        <p className="text-xs text-error mb-2">Mermaid diagram error</p>
        <pre className="text-xs text-low overflow-auto">
          <code>{chart}</code>
        </pre>
      </div>
    );
  }

  if (!svg) {
    return (
      <div className="flex items-center justify-center p-base text-low text-sm">
        Loading diagram…
      </div>
    );
  }

  return (
    <div
      className="relative my-3 flex justify-center overflow-auto"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <div
        className="cursor-zoom-in"
        role="button"
        tabIndex={0}
        onClick={openPreview}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            openPreview();
          }
        }}
        dangerouslySetInnerHTML={{ __html: svg }}
      />
      {isHovered && (
        <button
          type="button"
          className="absolute right-1 top-1 rounded-sm border border-border bg-primary/90 p-1.5 text-low shadow-sm hover:bg-secondary hover:text-high"
          onClick={(e) => {
            e.stopPropagation();
            openPreview();
          }}
          title="放大查看"
        >
          <Maximize2 className="w-3.5 h-3.5" />
        </button>
      )}
    </div>
  );
}
