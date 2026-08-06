"use client";

/**
 * Pastilles de messages suggérés, sous le champ de saisie. Un clic
 * pré-remplit le champ (l'utilisateur relit et modifie avant d'envoyer).
 */
export function MessageSuggestions({
  suggestions,
  onPick,
  label = "Suggestions de message",
}: {
  suggestions: string[];
  onPick: (suggestion: string) => void;
  label?: string;
}) {
  if (suggestions.length === 0) return null;
  return (
    <div
      role="group"
      aria-label={label}
      className="-mx-1 flex gap-2 overflow-x-auto px-1 pb-0.5 [scrollbar-width:none]"
    >
      {suggestions.map((suggestion) => (
        <button
          key={suggestion}
          type="button"
          onClick={() => onPick(suggestion)}
          className="shrink-0 cursor-pointer rounded-full border border-neutre-300 bg-creme px-3 py-1.5 text-xs text-encre transition-colors hover:border-terracotta-500 hover:bg-terracotta-100"
        >
          {suggestion}
        </button>
      ))}
    </div>
  );
}
