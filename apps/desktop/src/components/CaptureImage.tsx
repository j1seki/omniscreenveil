import { useLayoutEffect, useRef } from "react";
import type { ImgHTMLAttributes } from "react";

export function CaptureImage({ image, ...props }: { image: Blob } & Omit<ImgHTMLAttributes<HTMLImageElement>, "src">) {
  const element = useRef<HTMLImageElement>(null);
  const offer = useRef<(image: Blob) => void>();

  useLayoutEffect(() => {
    const img = element.current;
    if (!img) return;
    let displayed: string | undefined;
    let decoding: string | undefined;
    let pending: Blob | undefined;
    let mounted = true;

    async function present() {
      if (!img || decoding || !pending) return;
      const url = URL.createObjectURL(pending);
      pending = undefined;
      decoding = url;
      const candidate = new Image();
      candidate.src = url;
      try {
        await candidate.decode();
        if (mounted) {
          img.src = url;
          if (displayed) URL.revokeObjectURL(displayed);
          displayed = url;
        }
      } catch { /* Keep the last decoded frame. */ }
      if (!mounted) return;
      if (displayed !== url) URL.revokeObjectURL(url);
      decoding = undefined;
      void present();
    }

    offer.current = (next) => {
      pending = next;
      void present();
    };
    return () => {
      mounted = false;
      offer.current = undefined;
      pending = undefined;
      img.removeAttribute("src");
      if (displayed) URL.revokeObjectURL(displayed);
      if (decoding) URL.revokeObjectURL(decoding);
    };
  }, []);

  useLayoutEffect(() => offer.current?.(image), [image]);

  return <img ref={element} {...props} />;
}
