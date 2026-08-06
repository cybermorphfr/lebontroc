"use client";

import { useEffect } from "react";

/** Panneau bas mobile-first du DS : overlay + panneau sur-arrondi. */
export function BottomSheet({
  open,
  onClose,
  title,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = "";
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[60] flex items-end justify-center sm:items-center">
      <button
        aria-label="Fermer"
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-encre/40"
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className="relative max-h-[80vh] w-full overflow-y-auto rounded-t-[32px] bg-creme p-5 pb-8 shadow-lg sm:max-w-md sm:rounded-[32px] sm:pb-5"
      >
        <div aria-hidden className="mx-auto mb-3 h-1 w-10 rounded-full bg-neutre-300 sm:hidden" />
        {title ? <h2 className="mb-3 font-display text-xl">{title}</h2> : null}
        {children}
      </div>
    </div>
  );
}
