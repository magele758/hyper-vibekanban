/**
 * Conversation Virtualizer Hook
 *
 * Shared TanStack Virtual configuration for the conversation list.
 * Owns the virtualizer instance, measurement, and imperative scroll helpers.
 */

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type RefObject,
} from 'react';
import {
  useVirtualizer,
  measureElement as defaultMeasureElement,
} from '@tanstack/react-virtual';
import type { Virtualizer, VirtualItem } from '@tanstack/react-virtual';

import {
  type ConversationRow,
  SIZE_ESTIMATE_PX,
  estimateSizeForRow,
  findPreviousUserMessageIndex,
} from './conversation-row-model';
import {
  NEAR_BOTTOM_THRESHOLD_PX,
  isAtBottom,
  isNearBottom,
} from './conversation-scroll-commands';

// TanStack Virtual's ScrollBehavior ('auto' | 'smooth' | 'instant') shadows
// the DOM ScrollBehavior. Use a narrow type to avoid TS2322 mismatches.
type ScrollToOptionsBehavior = 'auto' | 'smooth';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Number of items to render beyond the visible area in each direction. */
const OVERSCAN = 8;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ConversationVirtualizerOptions {
  /** The semantic row model driving the list (virtualized head only). */
  rows: ConversationRow[];

  /**
   * Total number of conversation rows (virtualized + unvirtualized tail).
   * The bottom-lock correction must fire when ANY row is added — including
   * unvirtualized tail rows that don't change `rows.length` or `totalSize`.
   * Without this, streaming entries appended to the tail silently grow the
   * scroll container while the correction never fires.
   */
  totalRowCount: number;

  /** Ref to the scrollable container element. */
  scrollContainerRef: RefObject<HTMLDivElement | null>;

  /**
   * Called when the at-bottom state changes. Shells use this to show/hide
   * the scroll-to-bottom affordance.
   */
  onAtBottomChange?: (atBottom: boolean) => void;

  shouldSuppressSizeAdjustment?: () => boolean;

  /**
   * Bumps whenever conversation content changes (including in-place tail
   * growth during streaming). Used to re-run bottom-lock correction.
   */
  contentVersion?: number;
}

export interface ConversationVirtualizerResult {
  /** The TanStack Virtual virtualizer instance. */
  virtualizer: Virtualizer<HTMLDivElement, Element>;

  /** Virtual items currently in the render window (including overscan). */
  virtualItems: VirtualItem[];

  /** Total pixel size of all items (for the scroll spacer). */
  totalSize: number;

  /**
   * Ref callback for row DOM elements. Attach to each rendered row's
   * container element alongside `data-index={virtualItem.index}`.
   * TanStack Virtual uses this to measure real DOM heights and attach
   * a ResizeObserver for automatic re-measurement on size changes.
   */
  measureElement: (node: Element | null) => void;

  /** Scroll to the absolute bottom of the list. */
  scrollToBottom: (behavior?: ScrollToOptionsBehavior) => void;

  /** Scroll to a specific row index. */
  scrollToIndex: (
    index: number,
    options?: {
      align?: 'start' | 'center' | 'end';
      behavior?: ScrollToOptionsBehavior;
    }
  ) => void;

  /**
   * Scroll to the previous user message relative to the first visible item.
   * Returns true if a target was found and scrolled to, false otherwise.
   */
  scrollToPreviousUserMessage: () => boolean;

  /**
   * Whether the scroll container is currently near the bottom.
   * Reactive — updates via scroll event listener, not just point-in-time.
   */
  isAtBottom: boolean;

  /** Point-in-time check (non-reactive). Reads DOM directly. */
  checkIsAtBottom: () => boolean;

  /**
   * Whether new content should auto-scroll to the bottom. True when the
   * bottom-lock is active or the reader is already near the bottom.
   */
  shouldStickToBottom: () => boolean;

  /**
   * Release the bottom-lock. Call when navigating away from the
   * bottom (e.g., scrollToPreviousUserMessage).
   */
  releaseBottomLock: () => void;

  /**
   * Look up the ConversationRow index for a given virtual item.
   * Since our virtualizer uses identity mapping (no lane reordering),
   * this is simply `virtualItem.index`.
   */
  rowIndexForVirtualItem: (item: VirtualItem) => number;

  /**
   * Look up the ConversationRow for a given virtual item.
   * Returns undefined if the index is out of bounds.
   */
  rowForVirtualItem: (item: VirtualItem) => ConversationRow | undefined;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Configure and return a TanStack Virtual virtualizer for the conversation list.
 *
 * This hook is the single source of virtualizer configuration. It is consumed
 * by `ConversationListContainer` and must not be duplicated across shells.
 */
export function useConversationVirtualizer({
  rows,
  totalRowCount,
  scrollContainerRef,
  onAtBottomChange,
  shouldSuppressSizeAdjustment,
  contentVersion = 0,
}: ConversationVirtualizerOptions): ConversationVirtualizerResult {
  const bottomLockedRef = useRef(false);
  const userDetachedFromBottomRef = useRef(false);
  const userScrollUpDeadlineRef = useRef(0);
  const smoothScrollDeadlineRef = useRef(0);

  const isBottomScrollCorrectionActive = useCallback(
    () => bottomLockedRef.current,
    []
  );

  // -------------------------------------------------------------------------
  // Virtualizer instance
  // -------------------------------------------------------------------------

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: (index) => {
      const row = rows[index];
      if (!row) return SIZE_ESTIMATE_PX.medium;
      const containerWidth = scrollContainerRef.current?.clientWidth ?? null;
      return estimateSizeForRow(row, containerWidth);
    },
    getItemKey: (index) => {
      const row = rows[index];
      return row ? row.semanticKey : index;
    },
    overscan: OVERSCAN,
    measureElement: defaultMeasureElement,
    useAnimationFrameWithResizeObserver: false,
  });

  // -------------------------------------------------------------------------
  // shouldAdjustScrollPositionOnItemSizeChange
  //
  // Preserve the reader's position only when a row fully above the viewport
  // changes size. Mid-list flicker happens when we compensate for rows that
  // are still visible or below the viewport, because those corrections can
  // move the render window and trigger another measurement pass.
  // -------------------------------------------------------------------------

  useEffect(() => {
    virtualizer.shouldAdjustScrollPositionOnItemSizeChange = (
      item,
      _delta,
      instance
    ) => {
      const scrollElement = scrollContainerRef.current;
      const viewportHeight =
        scrollElement?.clientHeight ?? instance.scrollRect?.height ?? 0;
      const scrollOffset =
        scrollElement?.scrollTop ?? instance.scrollOffset ?? 0;
      const totalScrollableSize =
        scrollElement?.scrollHeight ?? instance.getTotalSize();
      const remainingDistance =
        totalScrollableSize - (scrollOffset + viewportHeight);
      const isItemFullyAboveViewport = item.end <= scrollOffset;
      const isBottomLocked = bottomLockedRef.current;

      const shouldAdjust =
        !isBottomLocked &&
        !shouldSuppressSizeAdjustment?.() &&
        isItemFullyAboveViewport &&
        remainingDistance > NEAR_BOTTOM_THRESHOLD_PX;

      return shouldAdjust;
    };

    return () => {
      virtualizer.shouldAdjustScrollPositionOnItemSizeChange = undefined;
    };
  }, [shouldSuppressSizeAdjustment, virtualizer]);

  // -------------------------------------------------------------------------
  // Reactive isAtBottom state
  // -------------------------------------------------------------------------

  const [isAtBottomState, setIsAtBottomState] = useState(true);
  const onAtBottomChangeRef = useRef(onAtBottomChange);
  onAtBottomChangeRef.current = onAtBottomChange;
  const lastAtBottomRef = useRef(true);

  const syncIsAtBottom = useCallback(() => {
    const el = scrollContainerRef.current;
    const nextValue = isBottomScrollCorrectionActive()
      ? true
      : el
        ? isNearBottom(el.scrollTop, el.clientHeight, el.scrollHeight)
        : true;

    if (nextValue !== lastAtBottomRef.current) {
      lastAtBottomRef.current = nextValue;
      setIsAtBottomState(nextValue);
      onAtBottomChangeRef.current?.(nextValue);
      return;
    }

    setIsAtBottomState((current) =>
      current === nextValue ? current : nextValue
    );
  }, [isBottomScrollCorrectionActive, scrollContainerRef]);

  const prevScrollTopRef = useRef(0);

  const snapToBottomIfLocked = useCallback(() => {
    if (!bottomLockedRef.current) return;
    if (performance.now() < smoothScrollDeadlineRef.current) return;
    if (performance.now() < userScrollUpDeadlineRef.current) return;

    const el = scrollContainerRef.current;
    if (!el) return;

    const maxScroll = el.scrollHeight - el.clientHeight;
    if (maxScroll > 0 && Math.abs(maxScroll - el.scrollTop) > 1) {
      el.scrollTop = maxScroll;
    }
  }, [scrollContainerRef]);

  const engageBottomLock = useCallback(() => {
    bottomLockedRef.current = true;
    userDetachedFromBottomRef.current = false;
    userScrollUpDeadlineRef.current = 0;
  }, []);

  const detachFromBottom = useCallback(() => {
    bottomLockedRef.current = false;
    userDetachedFromBottomRef.current = true;
    userScrollUpDeadlineRef.current = performance.now() + 200;
  }, []);

  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el) return;

    prevScrollTopRef.current = el.scrollTop;

    const handleScroll = () => {
      const currentScrollTop = el.scrollTop;
      const atBottom = isAtBottom(
        currentScrollTop,
        el.clientHeight,
        el.scrollHeight
      );
      const canChangeBottomLock =
        performance.now() > smoothScrollDeadlineRef.current &&
        performance.now() > userScrollUpDeadlineRef.current &&
        !shouldSuppressSizeAdjustment?.();
      const scrolledUp = prevScrollTopRef.current - currentScrollTop > 2;
      const scrolledDown = currentScrollTop - prevScrollTopRef.current > 2;

      // Release bottom lock on user-initiated upward scroll.
      if (bottomLockedRef.current && scrolledUp && canChangeBottomLock) {
        detachFromBottom();
      } else if (
        userDetachedFromBottomRef.current &&
        !bottomLockedRef.current &&
        atBottom &&
        scrolledDown &&
        canChangeBottomLock
      ) {
        // Reader scrolled back to the exact bottom — resume auto-follow.
        engageBottomLock();
        snapToBottomIfLocked();
      }

      prevScrollTopRef.current = currentScrollTop;
      syncIsAtBottom();
    };

    const handleWheel = (event: WheelEvent) => {
      if (event.deltaY >= 0 || !bottomLockedRef.current) return;
      if (performance.now() < smoothScrollDeadlineRef.current) return;
      if (shouldSuppressSizeAdjustment?.()) return;

      // Release lock on upward wheel intent before scroll position catches up,
      // so bottom-lock correction does not fight the first drag.
      detachFromBottom();
    };

    el.addEventListener('scroll', handleScroll, { passive: true });
    el.addEventListener('wheel', handleWheel, { passive: true });
    handleScroll();

    return () => {
      el.removeEventListener('scroll', handleScroll);
      el.removeEventListener('wheel', handleWheel);
    };
  }, [
    detachFromBottom,
    engageBottomLock,
    scrollContainerRef,
    shouldSuppressSizeAdjustment,
    snapToBottomIfLocked,
    syncIsAtBottom,
  ]);

  // -------------------------------------------------------------------------
  // Derived state
  // -------------------------------------------------------------------------

  const virtualItems = virtualizer.getVirtualItems();
  const totalSize = virtualizer.getTotalSize();

  useLayoutEffect(() => {
    syncIsAtBottom();
    snapToBottomIfLocked();
  }, [
    rows.length,
    totalRowCount,
    totalSize,
    contentVersion,
    snapToBottomIfLocked,
    syncIsAtBottom,
  ]);

  // -------------------------------------------------------------------------
  // Imperative helpers
  // -------------------------------------------------------------------------

  const scrollToBottom = useCallback(
    (behavior: ScrollToOptionsBehavior = 'smooth') => {
      const el = scrollContainerRef.current;
      if (!el) return;

      engageBottomLock();

      if (behavior === 'smooth') {
        smoothScrollDeadlineRef.current = performance.now() + 500;
        el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
      } else {
        el.scrollTop = el.scrollHeight - el.clientHeight;
      }
    },
    [engageBottomLock, scrollContainerRef]
  );

  const scrollToIndex = useCallback(
    (
      index: number,
      options?: {
        align?: 'start' | 'center' | 'end';
        behavior?: ScrollToOptionsBehavior;
      }
    ) => {
      if (bottomLockedRef.current) {
        detachFromBottom();
      }

      virtualizer.scrollToIndex(index, {
        align: options?.align ?? 'start',
        behavior: options?.behavior ?? 'smooth',
      });
    },
    [detachFromBottom, virtualizer]
  );

  const scrollToPreviousUserMessage = useCallback((): boolean => {
    const scrollEl = scrollContainerRef.current;
    const items = virtualizer.getVirtualItems();
    if (items.length === 0 || rows.length === 0 || !scrollEl) return false;

    const firstVisibleIndex =
      virtualizer.getVirtualItemForOffset(scrollEl.scrollTop)?.index ??
      items[0].index;
    const targetIndex = findPreviousUserMessageIndex(rows, firstVisibleIndex);

    if (targetIndex < 0) return false;

    virtualizer.scrollToIndex(targetIndex, {
      align: 'start',
      behavior: 'smooth',
    });
    return true;
  }, [scrollContainerRef, virtualizer, rows]);

  const checkIsAtBottom = useCallback((): boolean => {
    const el = scrollContainerRef.current;
    if (!el) return true;
    return isNearBottom(el.scrollTop, el.clientHeight, el.scrollHeight);
  }, [scrollContainerRef]);

  const shouldStickToBottom = useCallback((): boolean => {
    if (userDetachedFromBottomRef.current) return false;
    if (bottomLockedRef.current) return true;
    return checkIsAtBottom();
  }, [checkIsAtBottom]);

  const releaseBottomLock = useCallback(() => {
    if (!bottomLockedRef.current && userDetachedFromBottomRef.current) return;
    detachFromBottom();
  }, [detachFromBottom]);

  // -------------------------------------------------------------------------
  // Row ↔ VirtualItem mapping
  // -------------------------------------------------------------------------

  const rowIndexForVirtualItem = useCallback(
    (item: VirtualItem): number => item.index,
    []
  );

  const rowForVirtualItem = useCallback(
    (item: VirtualItem): ConversationRow | undefined => rows[item.index],
    [rows]
  );

  const measureElement = useCallback(
    (node: Element | null) => {
      virtualizer.measureElement(node);
    },
    [virtualizer]
  );

  // -------------------------------------------------------------------------
  // Return
  // -------------------------------------------------------------------------

  return {
    virtualizer,
    virtualItems,
    totalSize,
    measureElement,
    scrollToBottom,
    scrollToIndex,
    scrollToPreviousUserMessage,
    isAtBottom: isAtBottomState,
    checkIsAtBottom,
    shouldStickToBottom,
    releaseBottomLock,
    rowIndexForVirtualItem,
    rowForVirtualItem,
  };
}
