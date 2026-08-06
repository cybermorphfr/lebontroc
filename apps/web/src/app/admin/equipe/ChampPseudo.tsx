"use client";

import { useEffect, useRef, useState } from "react";

import { chercherPseudos, type Suggestion } from "./actions";

/**
 * Champ pseudo avec autocomplétion : chercher un membre ne doit pas
 * demander de connaître son orthographe exacte. Le backend résout par
 * égalité stricte — une faute de frappe donnait un échec silencieux.
 */
export function ChampPseudo({ name = "pseudo" }: { name?: string }) {
  const [valeur, setValeur] = useState("");
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [ouvert, setOuvert] = useState(false);
  const conteneur = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (valeur.trim().length < 2) {
      setSuggestions([]);
      return;
    }
    let annule = false;
    // Petite temporisation : on ne cherche pas à chaque frappe.
    const minuteur = setTimeout(async () => {
      const trouves = await chercherPseudos(valeur);
      if (!annule) {
        setSuggestions(trouves);
        setOuvert(true);
      }
    }, 200);
    return () => {
      annule = true;
      clearTimeout(minuteur);
    };
  }, [valeur]);

  // Un clic ailleurs referme la liste.
  useEffect(() => {
    function dehors(event: MouseEvent) {
      if (conteneur.current && !conteneur.current.contains(event.target as Node)) {
        setOuvert(false);
      }
    }
    document.addEventListener("mousedown", dehors);
    return () => document.removeEventListener("mousedown", dehors);
  }, []);

  return (
    <div ref={conteneur} className="relative">
      <input
        name={name}
        value={valeur}
        onChange={(e) => setValeur(e.target.value)}
        onFocus={() => suggestions.length > 0 && setOuvert(true)}
        placeholder="pseudo ou e-mail"
        aria-label="Pseudo du membre"
        autoComplete="off"
        required
        className="w-56 rounded-full border border-neutre-300 bg-creme px-3.5 py-2 text-sm outline-none focus:border-terracotta-500"
      />
      {ouvert && suggestions.length > 0 ? (
        <ul className="absolute z-10 mt-1 flex w-72 flex-col overflow-hidden rounded-3xl border border-neutre-300 bg-creme shadow-lg">
          {suggestions.map((s) => (
            <li key={s.pseudo}>
              <button
                type="button"
                onClick={() => {
                  setValeur(s.pseudo);
                  setOuvert(false);
                }}
                className="flex w-full cursor-pointer items-center gap-2 px-4 py-2 text-left text-sm hover:bg-sable"
              >
                <span className="font-semibold">{s.pseudo}</span>
                <span className="text-xs text-neutre-700">
                  {s.role !== "utilisateur" ? `${s.role} · ` : ""}
                  {s.annonces} annonce{s.annonces > 1 ? "s" : ""}
                </span>
                {s.is_master ? <span className="ml-auto text-xs">🔒</span> : null}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
