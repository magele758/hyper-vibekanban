import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
  type WheelEvent,
} from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import { create, useModal } from '@ebay/nice-modal-react';
import { Minus, Plus, RotateCcw } from 'lucide-react';
import { defineModal } from '@/shared/lib/modals';
import { useMermaidRender } from '@/shared/hooks/useMermaidRender';

export interface MermaidPreviewDialogProps {
  chart: string;
  theme: 'light' | 'dark';
}

const MIN_ZOOM = 0.25;
const MAX_ZOOM = 4;
const ZOOM_STEP = 0.25;
const FIT_PADDING = 24;

interface SvgDimensions {
  width: number;
  height: number;
}

function clampZoom(value: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value));
}

function parseSvgDimensions(svg: string): SvgDimensions | null {
  const viewBoxMatch = svg.match(/viewBox=["']([^"']+)["']/i);
  if (viewBoxMatch) {
    const values = viewBoxMatch[1].trim().split(/[\s,]+/).map(Number);
    if (values.length === 4 && values[2] > 0 && values[3] > 0) {
      return { width: values[2], height: values[3] };
    }
  }

  const widthMatch = svg.match(/\bwidth=["']([\d.]+)/i);
  const heightMatch = svg.match(/\bheight=["']([\d.]+)/i);
  const width = widthMatch ? Number.parseFloat(widthMatch[1]) : 0;
  const height = heightMatch ? Number.parseFloat(heightMatch[1]) : 0;
  if (width > 0 && height > 0) {
    return { width, height };
  }

  return null;
}

function preparePreviewSvg(svg: string, dimensions: SvgDimensions): string {
  return svg.replace(/<svg\b([^>]*)>/i, (_match, attrs: string) => {
    const cleaned = attrs
      .replace(/\bwidth=["'][^"']*["']/gi, '')
      .replace(/\bheight=["'][^"']*["']/gi, '')
      .replace(/\bstyle=["'][^"']*["']/gi, '');
    return `<svg${cleaned} width="${dimensions.width}" height="${dimensions.height}" style="display:block">`;
  });
}

function computeFitScale(
  viewport: HTMLElement,
  dimensions: SvgDimensions
): number {
  const availableWidth = Math.max(viewport.clientWidth - FIT_PADDING * 2, 1);
  const availableHeight = Math.max(viewport.clientHeight - FIT_PADDING * 2, 1);
  return Math.min(
    availableWidth / dimensions.width,
    availableHeight / dimensions.height
  );
}

const MermaidPreviewDialogImpl = create<MermaidPreviewDialogProps>((props) => {
  const modal = useModal();
  const { chart, theme } = props;
  const { svg, error } = useMermaidRender(chart, theme);

  const viewportRef = useRef<HTMLDivElement>(null);
  const [fitScale, setFitScale] = useState(1);
  const [zoom, setZoom] = useState(1);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const dragOrigin = useRef<{
    x: number;
    y: number;
    posX: number;
    posY: number;
  } | null>(null);

  const svgDimensions = useMemo(
    () => (svg ? parseSvgDimensions(svg) : null),
    [svg]
  );
  const previewSvg = useMemo(() => {
    if (!svg || !svgDimensions) return svg;
    return preparePreviewSvg(svg, svgDimensions);
  }, [svg, svgDimensions]);

  const updateFitScale = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport || !svgDimensions) return;
    if (viewport.clientWidth === 0 || viewport.clientHeight === 0) return;

    setFitScale(computeFitScale(viewport, svgDimensions));
  }, [svgDimensions]);

  useLayoutEffect(() => {
    if (!modal.visible || !svg || error || !svgDimensions) return;

    setZoom(1);
    setPosition({ x: 0, y: 0 });

    let frame = 0;
    const measure = () => {
      const viewport = viewportRef.current;
      if (!viewport) return;
      if (viewport.clientWidth === 0 || viewport.clientHeight === 0) {
        frame = requestAnimationFrame(measure);
        return;
      }
      setFitScale(computeFitScale(viewport, svgDimensions));
    };
    measure();

    return () => {
      if (frame) cancelAnimationFrame(frame);
    };
  }, [modal.visible, svg, error, svgDimensions]);

  useEffect(() => {
    if (!modal.visible || !svg || error || !svgDimensions) return;

    const viewport = viewportRef.current;
    if (!viewport) return;

    const observer = new ResizeObserver(() => {
      updateFitScale();
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [modal.visible, svg, error, svgDimensions, updateFitScale]);

  const handleClose = () => modal.hide();

  const zoomIn = () => setZoom((value) => clampZoom(value + ZOOM_STEP));
  const zoomOut = () => setZoom((value) => clampZoom(value - ZOOM_STEP));
  const resetView = () => {
    setZoom(1);
    setPosition({ x: 0, y: 0 });
    updateFitScale();
  };

  const handleWheel = useCallback((e: WheelEvent<HTMLDivElement>) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
    setZoom((value) => clampZoom(value + delta));
  }, []);

  const displayScale = fitScale * zoom;

  const handleMouseDown = (e: MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    dragOrigin.current = {
      x: e.clientX,
      y: e.clientY,
      posX: position.x,
      posY: position.y,
    };
    setIsDragging(true);
  };

  const handleMouseMove = (e: MouseEvent<HTMLDivElement>) => {
    if (!dragOrigin.current) return;
    const { x, y, posX, posY } = dragOrigin.current;
    setPosition({ x: posX + (e.clientX - x), y: posY + (e.clientY - y) });
  };

  const stopDragging = () => {
    dragOrigin.current = null;
    setIsDragging(false);
  };

  return (
    <Dialog
      open={modal.visible}
      onOpenChange={handleClose}
      size="fullscreen"
      className="gap-0 p-0"
    >
      <DialogContent className="flex flex-col h-full min-h-0 p-0 overflow-hidden gap-0">
        <DialogHeader className="flex flex-row items-center justify-between gap-4 px-4 py-3 border-b border-border shrink-0 space-y-0">
          <DialogTitle className="text-sm">Mermaid 图表</DialogTitle>
          <div className="flex items-center gap-1 mr-6">
            <button
              type="button"
              className="rounded-sm p-1.5 text-low hover:bg-secondary hover:text-high"
              onClick={zoomOut}
              title="缩小"
            >
              <Minus className="w-4 h-4" />
            </button>
            <span className="text-xs text-low w-12 text-center select-none">
              {Math.round(zoom * 100)}%
            </span>
            <button
              type="button"
              className="rounded-sm p-1.5 text-low hover:bg-secondary hover:text-high"
              onClick={zoomIn}
              title="放大"
            >
              <Plus className="w-4 h-4" />
            </button>
            <button
              type="button"
              className="rounded-sm p-1.5 text-low hover:bg-secondary hover:text-high"
              onClick={resetView}
              title="重置"
            >
              <RotateCcw className="w-4 h-4" />
            </button>
          </div>
        </DialogHeader>
        <div
          ref={viewportRef}
          className={`relative flex-1 min-h-0 overflow-hidden bg-primary ${
            isDragging ? 'cursor-grabbing' : 'cursor-grab'
          }`}
          onWheel={handleWheel}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={stopDragging}
          onMouseLeave={stopDragging}
        >
          {error && (
            <pre className="absolute inset-0 overflow-auto p-base text-xs font-ibm-plex-mono text-normal">
              <code>{chart}</code>
            </pre>
          )}
          {!error && !svg && (
            <div className="absolute inset-0 flex items-center justify-center text-low text-sm">
              Loading diagram…
            </div>
          )}
          {!error && previewSvg && svgDimensions && (
            <div
              className="absolute top-1/2 left-1/2 select-none [&_svg]:block [&_svg]:max-w-none [&_svg]:max-h-none"
              style={{
                width: svgDimensions.width,
                height: svgDimensions.height,
                transform: `translate(-50%, -50%) translate(${position.x}px, ${position.y}px) scale(${displayScale})`,
                transformOrigin: 'center center',
              }}
              dangerouslySetInnerHTML={{ __html: previewSvg }}
            />
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
});

export const MermaidPreviewDialog = defineModal<
  MermaidPreviewDialogProps,
  void
>(MermaidPreviewDialogImpl);
