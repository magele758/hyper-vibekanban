import {
  useCallback,
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

const MIN_SCALE = 0.25;
const MAX_SCALE = 4;
const SCALE_STEP = 0.25;

function clampScale(value: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, value));
}

const MermaidPreviewDialogImpl = create<MermaidPreviewDialogProps>((props) => {
  const modal = useModal();
  const { chart, theme } = props;
  const { svg, error } = useMermaidRender(chart, theme);

  const [scale, setScale] = useState(1);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const dragOrigin = useRef<{
    x: number;
    y: number;
    posX: number;
    posY: number;
  } | null>(null);

  const handleClose = () => modal.hide();

  const zoomIn = () => setScale((s) => clampScale(s + SCALE_STEP));
  const zoomOut = () => setScale((s) => clampScale(s - SCALE_STEP));
  const resetView = () => {
    setScale(1);
    setPosition({ x: 0, y: 0 });
  };

  const handleWheel = useCallback((e: WheelEvent<HTMLDivElement>) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? -SCALE_STEP : SCALE_STEP;
    setScale((s) => clampScale(s + delta));
  }, []);

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
      className="w-[95vw] max-w-[1400px] h-[88vh] max-h-[88vh] p-0 overflow-hidden"
    >
      <DialogContent className="flex flex-col h-full p-0 overflow-hidden gap-0">
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
              {Math.round(scale * 100)}%
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
          className={`relative flex-1 overflow-hidden bg-primary ${
            isDragging ? 'cursor-grabbing' : 'cursor-grab'
          }`}
          onWheel={handleWheel}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={stopDragging}
          onMouseLeave={stopDragging}
        >
          {error && (
            <div className="absolute inset-0 flex items-center justify-center p-base text-error text-sm">
              {error}
            </div>
          )}
          {!error && !svg && (
            <div className="absolute inset-0 flex items-center justify-center text-low text-sm">
              Loading diagram…
            </div>
          )}
          {!error && svg && (
            <div
              className="absolute top-1/2 left-1/2 select-none [&_svg]:max-w-none"
              style={{
                transform: `translate(-50%, -50%) translate(${position.x}px, ${position.y}px) scale(${scale})`,
                transformOrigin: 'center center',
              }}
              dangerouslySetInnerHTML={{ __html: svg }}
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
