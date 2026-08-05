import Link from "next/link";

const ICONES: Record<string, string> = {
  "Enfants et puériculture": "🧸",
  Meubles: "🪑",
  Électroménager: "🫧",
  "High-tech": "📱",
  Vêtements: "👕",
  "Loisirs et sport": "🚴",
  "Maison et déco": "🏺",
  Autre: "📦",
};

/** Rail de chips catégories (pattern Leboncoin/Vinted) → recherche filtrée. */
export function CategoryChips({ roots }: { roots: { id: number; label: string }[] }) {
  return (
    <nav
      aria-label="Catégories"
      className="-mx-6 flex gap-2 overflow-x-auto px-6 pb-1 [scrollbar-width:none]"
    >
      {roots.map((root) => (
        <Link
          key={root.id}
          href={`/recherche?categorie=${root.id}`}
          className="flex shrink-0 items-center gap-1.5 rounded-full bg-sable px-4 py-2 text-sm font-semibold text-encre shadow-sm transition-colors hover:bg-creme"
        >
          <span aria-hidden>{ICONES[root.label] ?? "🔎"}</span>
          {root.label}
        </Link>
      ))}
    </nav>
  );
}
