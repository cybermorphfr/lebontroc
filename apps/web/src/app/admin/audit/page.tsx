export const dynamic = "force-dynamic";

// Journal d'audit immuable (F6.1).

const API = process.env.API_INTERNAL_URL ?? "http://localhost:8080";
const TOKEN = process.env.ADMIN_TOKEN ?? "";

type Entry = {
  id: number;
  action: string;
  target_type: string;
  target_id: string;
  details: string | null;
  created_at: string;
};

export default async function AdminAuditPage() {
  const response = await fetch(`${API}/admin/audit`, {
    headers: { "X-Admin-Token": TOKEN },
    cache: "no-store",
  });
  const entries: Entry[] = response.ok ? await response.json() : [];

  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-2 p-6 font-mono text-sm">
      <h1 className="text-xl font-bold">Journal d&apos;audit</h1>
      <a href="/admin" className="text-xs underline">
        ← Admin
      </a>
      {entries.map((e) => (
        <p key={e.id}>
          #{e.id} · {new Date(e.created_at).toLocaleString("fr-FR")} · {e.action} ·{" "}
          {e.target_type} {e.target_id}
          {e.details ? ` · ${e.details}` : ""}
        </p>
      ))}
      {entries.length === 0 ? <p>Aucune action pour l&apos;instant.</p> : null}
    </main>
  );
}
