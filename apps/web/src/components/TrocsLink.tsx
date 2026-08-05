"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";

import { apiFetch } from "@/lib/client-api";
import { useRealtime } from "@/lib/realtime";

/**
 * Entrée « Mes trocs » du header — en pratique la messagerie : deux
 * enveloppes et le total de messages non lus, rafraîchi en temps réel.
 */
export function TrocsLink() {
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
      href="/trocs"
      aria-label={`Mes trocs et messages${unread > 0 ? ` (${unread} non lus)` : ""}`}
      className="relative flex size-9 items-center justify-center rounded-full text-encre transition-colors hover:bg-encre/7"
    >
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.4"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden
      >
        <rect width="16" height="13" x="6" y="4" rx="2" />
        <path d="m22 7-7.1 3.78c-.57.3-1.23.3-1.8 0L6 7" />
        <path d="M2 8v11c0 1.1.9 2 2 2h14" />
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
