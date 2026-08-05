"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";

import { apiFetch } from "@/lib/client-api";

type Notification = {
  id: string;
  type: string;
  title: string;
  body: string;
  link: string;
  read: boolean;
  created_at: string;
};

const ICONES: Record<string, string> = {
  proposition_recue: "🔁",
  proposition_acceptee: "🤝",
  proposition_cloturee: "📪",
  message_recu: "💬",
  paiement: "💶",
  expedition: "📦",
  remise: "🎉",
  evaluation: "⭐",
  litige: "⚖️",
  favori: "❤️",
};

/** Centre de notifications (F5.3). */
export default function NotificationsPage() {
  const router = useRouter();
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [loaded, setLoaded] = useState(false);

  const reload = useCallback(() => {
    apiFetch("/notifications")
      .then((r) => (r.ok ? r.json() : null))
      .then((data: { notifications: Notification[] } | null) => {
        if (data) setNotifications(data.notifications);
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  useEffect(reload, [reload]);

  async function open(notification: Notification) {
    if (!notification.read) {
      await apiFetch(`/notifications/${notification.id}/read`, { method: "POST" }).catch(() => {});
    }
    router.push(notification.link);
  }

  async function markAll() {
    await apiFetch("/notifications/read-all", { method: "POST" }).catch(() => {});
    reload();
  }

  const hasUnread = notifications.some((n) => !n.read);

  return (
    <main className="mx-auto flex w-full max-w-xl flex-col gap-4 px-6 pb-16">
      <div className="flex items-center justify-between gap-2">
        <h1 className="font-display text-2xl">Notifications</h1>
        <div className="flex items-center gap-3 text-xs">
          {hasUnread ? (
            <button onClick={markAll} className="cursor-pointer text-neutre-700 underline">
              Tout marquer comme lu
            </button>
          ) : null}
          <Link href="/reglages/notifications" className="text-neutre-700 underline">
            Réglages
          </Link>
        </div>
      </div>

      {loaded && notifications.length === 0 ? (
        <section className="flex flex-col gap-2 rounded-[32px] bg-sable p-6 shadow-sm">
          <h2 className="font-display text-lg">Rien pour l&apos;instant</h2>
          <p className="text-sm text-neutre-700">
            Les propositions, colis, paiements et évaluations de tes trocs apparaîtront ici.
          </p>
        </section>
      ) : (
        <ul className="flex flex-col gap-2">
          {notifications.map((notification) => (
            <li key={notification.id}>
              <button
                onClick={() => open(notification)}
                className={`flex w-full cursor-pointer items-start gap-3 rounded-3xl p-4 text-left shadow-sm transition-colors ${
                  notification.read ? "bg-sable/60" : "bg-sable hover:bg-creme"
                }`}
              >
                <span aria-hidden className="text-xl">
                  {ICONES[notification.type] ?? "🔔"}
                </span>
                <span className="flex flex-col gap-0.5">
                  <span className={`text-sm ${notification.read ? "" : "font-semibold"}`}>
                    {notification.title}
                  </span>
                  <span className="text-sm text-neutre-700">{notification.body}</span>
                  <span className="text-xs text-neutre-700">
                    {new Date(notification.created_at).toLocaleString("fr-FR", {
                      day: "numeric",
                      month: "long",
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </span>
                </span>
                {!notification.read ? (
                  <span
                    aria-label="Non lue"
                    className="ml-auto mt-1 size-2 shrink-0 rounded-full bg-[#c67139]"
                  />
                ) : null}
              </button>
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}
