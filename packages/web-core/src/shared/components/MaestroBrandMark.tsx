import { useEffect } from 'react';
import { cn } from '@/shared/lib/utils';
import { useTheme, getResolvedTheme } from '@/shared/hooks/useTheme';
import {
  LITE_PRODUCT_NAME,
  MAESTRO_FAVICON_DARK_SRC,
  MAESTRO_FAVICON_SRC,
  MAESTRO_ICON_SRC,
  MAESTRO_LOGO_DARK_SRC,
  MAESTRO_LOGO_SRC,
  isViteLiteMode,
} from '@/shared/lib/liteMode';

export function MaestroBrandMark({
  className,
  alt = LITE_PRODUCT_NAME,
}: {
  className?: string;
  alt?: string;
}) {
  return (
    <img
      src={MAESTRO_ICON_SRC}
      alt={alt}
      className={cn('h-7 w-7 rounded-md', className)}
    />
  );
}

export function MaestroBrandLogo({ className }: { className?: string }) {
  const { theme } = useTheme();
  const dark = getResolvedTheme(theme) === 'dark';

  return (
    <img
      src={dark ? MAESTRO_LOGO_DARK_SRC : MAESTRO_LOGO_SRC}
      alt={LITE_PRODUCT_NAME}
      className={cn('h-8 w-auto', className)}
    />
  );
}

/** Swap document favicons when the lite desktop flavor is active. */
export function MaestroFavicon() {
  useEffect(() => {
    if (!isViteLiteMode()) {
      return;
    }

    const mappings: Array<{
      selector: string;
      href: string;
    }> = [
      {
        selector: 'link[rel="icon"][media="(prefers-color-scheme: light)"]',
        href: MAESTRO_FAVICON_SRC,
      },
      {
        selector: 'link[rel="icon"][media="(prefers-color-scheme: dark)"]',
        href: MAESTRO_FAVICON_DARK_SRC,
      },
      {
        selector: 'link[rel="apple-touch-icon"]',
        href: MAESTRO_ICON_SRC,
      },
    ];

    for (const { selector, href } of mappings) {
      const el = document.querySelector<HTMLLinkElement>(selector);
      if (el) {
        el.href = href;
      }
    }

    const manifest = document.querySelector<HTMLLinkElement>(
      'link[rel="manifest"]'
    );
    if (manifest) {
      manifest.href = '/site-maestro.webmanifest';
    }
  }, []);

  return null;
}
