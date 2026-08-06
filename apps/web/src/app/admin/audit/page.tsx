import { adminFetch, adminPost } from "../adminFetch";
export const dynamic = "force-dynamic";

// Journal d'audit immuable (F6.1).


type Entry = {
  id: number;
  actor_pseudo: string | null;
  action: string;
  target_type: string;
  target_id: string;
  details: string | null;
  created_at: string;
};

export default async function AdminAuditPage() {
  const entries: Entry[] = (await adminFetch<Entry[]>(`/admin/audit`)) ?? [];

  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-2 p-6 font-mono text-sm">
      <h1 className="text-xl font-bold">Journal d&apos;audit</h1>
      <a href="/admin" className="text-xs underline">
        ← Admin
      </a>
      {entries.map((e) => (
        <p key={e.id}>
          #{e.id} · {new Date(e.created_at).toLocaleString("fr-FR")} ·{" "}
          {e.actor_pseudo ?? "service"} · {e.action} ·{" "}
          {e.target_type} {e.target_id}
          {e.details ? ` · ${e.details}` : ""}
        </p>
      ))}
      {entries.length === 0 ? <p>Aucune action pour l&apos;instant.</p> : null}
    </main>
  );
}
