"use client";

import { useEffect, useState } from "react";
import Link from "next/link";

import { apiFetch } from "@/lib/client-api";
import { useRealtime } from "@/lib/realtime";

/** Cloche du centre de notifications (F5.3) — badge temps réel via WS. */
export function NotificationBell() {
  const [unread, setUnread] = useState(0);

  useEffect(() => {
    apiFetch("/notifications/unread-count")
      .then((r) => (r.ok ? r.json() : null))
      .then((data: { unread_count: number } | null) => {
        if (data) setUnread(data.unread_count);
      })
      .catch(() => {});
  }, []);

  useRealtime((event) => {
    if (event.type === "notification_new") setUnread(event.unread_count);
  });

  return (
    <Link
      href="/notifications"
      aria-label={`Notifications${unread > 0 ? ` (${unread} non lues)` : ""}`}
      className="relative flex size-9 items-center justify-center rounded-full text-encre transition-colors hover:bg-encre/7"
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
        <path d="M10.268 21a2 2 0 0 0 3.464 0" />
        <path d="M3.262 15.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673C19.41 13.956 18 12.499 18 8A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326" />
      </svg>
      {unread > 0 ? (
        <span
          data-testid="badge-notifications"
          className="absolute -right-0.5 -top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-[#c67139] px-1 font-display text-[10px] leading-none text-creme"
        >
          {unread > 9 ? "9+" : unread}
        </span>
      ) : null}
    </Link>
  );
}
