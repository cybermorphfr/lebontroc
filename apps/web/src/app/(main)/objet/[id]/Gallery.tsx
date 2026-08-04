"use client";

import { useCallback, useEffect, useState } from "react";

import { apiFetch } from "@/lib/client-api";

/**
 * Galerie de la fiche objet : image principale + vignettes, plein écran au
 * clic (télémétrie `item_gallery_opened`).
 */
export function Gallery({
  itemId,
  title,
  photos,
}: {
  itemId: string;
  title: string;
  photos: string[];
}) {
  const [current, setCurrent] = useState(0);
  const [fullscreen, setFullscreen] = useState(false);

  const openFullscreen = useCallback(() => {
    setFullscreen(true);
    void apiFetch("/analytics/track", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "item_gallery_opened", item_id: itemId }),
    });
  }, [itemId]);

  useEffect(() => {
    if (!fullscreen) return;
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") setFullscreen(false);
      if (event.key === "ArrowRight") setCurrent((c) => (c + 1) % photos.length);
      if (event.key === "ArrowLeft") setCurrent((c) => (c - 1 + photos.length) % photos.length);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [fullscreen, photos.length]);

  if (photos.length === 0) {
    return <div className="aspect-square rounded-[32px] bg-neutre-100" aria-hidden />;
  }

  return (
    <>
      <div className="flex flex-col gap-2">
        <button
          onClick={openFullscreen}
          aria-label="Voir les photos en plein écran"
          className="cursor-zoom-in overflow-hidden rounded-[32px] bg-neutre-100"
        >
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src={photos[current]} alt={title} className="aspect-square w-full object-cover" />
        </button>
        {photos.length > 1 ? (
          <div className="flex gap-2 overflow-x-auto">
            {photos.map((url, index) => (
              <button
                key={url}
                onClick={() => setCurrent(index)}
                aria-label={`Photo ${index + 1}`}
                aria-current={index === current}
                className={`shrink-0 overflow-hidden rounded-2xl border-2 ${
                  index === current ? "border-terracotta-500" : "border-transparent"
                }`}
              >
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img src={url} alt="" className="size-16 object-cover" />
              </button>
            ))}
          </div>
        ) : null}
      </div>

      {fullscreen ? (
        <div
          role="dialog"
          aria-modal="true"
          aria-label={`Photos de ${title}`}
          className="fixed inset-0 z-50 flex items-center justify-center bg-encre/95"
          onClick={() => setFullscreen(false)}
        >
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            src={photos[current]}
            alt={title}
            className="max-h-full max-w-full object-contain"
            onClick={(e) => e.stopPropagation()}
          />
          <button
            onClick={() => setFullscreen(false)}
            aria-label="Fermer"
            className="absolute right-4 top-4 flex size-11 items-center justify-center rounded-full bg-creme/15 text-2xl text-creme"
          >
            ×
          </button>
          {photos.length > 1 ? (
            <>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setCurrent((c) => (c - 1 + photos.length) % photos.length);
                }}
                aria-label="Photo précédente"
                className="absolute left-4 flex size-11 items-center justify-center rounded-full bg-creme/15 text-2xl text-creme"
              >
                ‹
              </button>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setCurrent((c) => (c + 1) % photos.length);
                }}
                aria-label="Photo suivante"
                className="absolute right-4 flex size-11 items-center justify-center rounded-full bg-creme/15 text-2xl text-creme"
              >
                ›
              </button>
              <p className="absolute bottom-4 rounded-full bg-creme/15 px-3 py-1 text-sm text-creme">
                {current + 1} / {photos.length}
              </p>
            </>
          ) : null}
        </div>
      ) : null}
    </>
  );
}
