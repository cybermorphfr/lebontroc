"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";

import { apiFetch } from "@/lib/client-api";
import { useRealtime } from "@/lib/realtime";

/** Icône Messages du header (pattern Vinted) — badge de non-lus temps réel. */
export function MessagesBell() {
  const [unread, setUnread] = useState(0);

  const refresh = useCallback(() => {
    apiFetch("/me/conversations")
      .then((r) => (r.ok ? r.json() : null))
      .then((data: { unread_count: number }[] | null) => {
        if (data) setUnread(data.reduce((sum, c) => sum + c.unread_count, 0));
      })
      .catch(() => {});
  }, []);

  useEffect(refresh, [refresh]);
  useRealtime((event) => {
    if (event.type === "message" || event.type === "read") refresh();
  });

  return (
    <Link
      href="/messages"
      aria-label={`Messages${unread > 0 ? ` (${unread} non lus)` : ""}`}
      className="relative flex size-9 items-center justify-center rounded-full text-encre transition-colors hover:bg-encre/7"
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
        <path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z" />
      </svg>
      {unread > 0 ? (
        <span
          data-testid="badge-messages"
          className="absolute -right-0.5 -top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-[#c67139] px-1 font-display text-[10px] leading-none text-creme"
        >
          {unread > 9 ? "9+" : unread}
        </span>
      ) : null}
    </Link>
  );
}
