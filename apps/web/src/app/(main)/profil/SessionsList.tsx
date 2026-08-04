"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import type { SessionResponse } from "@lebontroc/api-client";

import { Button } from "@/components/ui/Button";
import { Tag } from "@/components/ui/Tag";
import { apiFetch } from "@/lib/client-api";

function ilYA(dateIso: string): string {
  const secondes = Math.max(0, Math.floor((Date.now() - new Date(dateIso).getTime()) / 1000));
  if (secondes < 60) return "à l'instant";
  const minutes = Math.floor(secondes / 60);
  if (minutes < 60) return `il y a ${minutes} min`;
  const heures = Math.floor(minutes / 60);
  if (heures < 24) return `il y a ${heures} h`;
  const jours = Math.floor(heures / 24);
  return `il y a ${jours} j`;
}

function nomAppareil(userAgent: string | null | undefined): string {
  if (!userAgent) return "Appareil inconnu";
  if (/mobile|android|iphone/i.test(userAgent)) return "Mobile";
  if (/firefox/i.test(userAgent)) return "Firefox";
  if (/edg/i.test(userAgent)) return "Edge";
  if (/chrome/i.test(userAgent)) return "Chrome";
  if (/safari/i.test(userAgent)) return "Safari";
  return "Navigateur";
}

export function SessionsList({ sessions }: { sessions: SessionResponse[] }) {
  const router = useRouter();
  const [message, setMessage] = useState<string | null>(null);

  async function revoke(id: string) {
    await apiFetch(`/auth/sessions/${id}`, { method: "DELETE" });
    router.refresh();
  }

  async function revokeOthers() {
    await apiFetch("/auth/sessions", { method: "DELETE" });
    setMessage("C'est fait. Seul cet appareil reste connecté.");
    router.refresh();
  }

  return (
    <div className="flex flex-col gap-3">
      <ul className="flex flex-col gap-2">
        {sessions.map((session) => (
          <li
            key={session.id}
            className="flex flex-wrap items-center justify-between gap-2 rounded-2xl bg-creme px-4 py-3"
          >
            <div className="flex flex-col">
              <span className="flex items-center gap-2 text-sm font-semibold">
                {nomAppareil(session.user_agent)}
                {session.current ? <Tag variant="accent-2">Cet appareil</Tag> : null}
              </span>
              <span className="text-xs text-neutre-700">
                Dernière activité {ilYA(session.last_used_at)}
              </span>
            </div>
            {!session.current ? (
              <Button variant="ghost" onClick={() => revoke(session.id)}>
                Déconnecter
              </Button>
            ) : null}
          </li>
        ))}
      </ul>
      {message ? <p className="text-sm text-sauge-700">{message}</p> : null}
      {sessions.some((s) => !s.current) ? (
        <div>
          <Button variant="secondary" onClick={revokeOthers}>
            Déconnecter tous les autres appareils
          </Button>
        </div>
      ) : null}
    </div>
  );
}
