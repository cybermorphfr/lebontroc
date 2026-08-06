"use client";

import { useEffect, useRef } from "react";

export type RealtimeEvent =
  | {
      type: "message";
      proposal_id: string;
      message: {
        id: string;
        proposal_id: string;
        sender_pseudo: string;
        body: string;
        photo_url: string | null;
        redacted: boolean;
        created_at: string;
        read_at: string | null;
      };
    }
  | { type: "read"; proposal_id: string; reader_pseudo: string }
  | {
      type: "trade_updated";
      proposal_id?: string;
      trade_id?: string;
      /** Contre-proposition : le fil a déménagé ici. */
      new_proposal_id?: string;
    }
  | { type: "notification_new"; unread_count: number };

/**
 * Connexion WebSocket au flux temps réel, avec reconnexion automatique
 * (backoff progressif plafonné à 15 s).
 */
export function useRealtime(onEvent: (event: RealtimeEvent) => void) {
  const handler = useRef(onEvent);
  handler.current = onEvent;

  useEffect(() => {
    let socket: WebSocket | null = null;
    let attempts = 0;
    let closed = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    function connect() {
      const protocol = window.location.protocol === "https:" ? "wss" : "ws";
      socket = new WebSocket(`${protocol}://${window.location.host}/api/ws`);
      socket.onopen = () => {
        attempts = 0;
      };
      socket.onmessage = (event) => {
        try {
          handler.current(JSON.parse(event.data as string) as RealtimeEvent);
        } catch {
          // trame illisible : ignorée
        }
      };
      socket.onclose = () => {
        if (closed) return;
        attempts += 1;
        timer = setTimeout(connect, Math.min(1000 * 2 ** attempts, 15000));
      };
    }

    connect();
    return () => {
      closed = true;
      if (timer) clearTimeout(timer);
      socket?.close();
    };
  }, []);
}
