import type { TextareaHTMLAttributes } from "react";

type TextareaProps = TextareaHTMLAttributes<HTMLTextAreaElement> & {
  label?: string;
  hint?: string;
  error?: string;
};

/** Zone de texte du DS — arrondi doux (pas pilule), label/hint/erreur. */
export function Textarea({ label, hint, error, id, className = "", ...rest }: TextareaProps) {
  const describedBy = error ? `${id}-erreur` : hint ? `${id}-hint` : undefined;
  return (
    <div className="flex flex-col">
      {label ? (
        <label htmlFor={id} className="mb-1.5 text-xs text-encre/70">
          {label}
        </label>
      ) : null}
      <textarea
        id={id}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        className={`min-h-24 w-full resize-y rounded-3xl border bg-sable px-4 py-2.5 text-sm text-encre caret-terracotta-500 transition-colors placeholder:text-neutre-500 hover:border-encre/45 focus-visible:border-terracotta-500 focus-visible:outline-none ${
          error ? "border-terracotta-500" : "border-neutre-300"
        } ${className}`}
        {...rest}
      />
      {error ? (
        <p id={`${id}-erreur`} className="mt-1 text-[11px] text-terracotta-700">
          {error}
        </p>
      ) : hint ? (
        <p id={`${id}-hint`} className="mt-1 text-[11px] text-neutre-700">
          {hint}
        </p>
      ) : null}
    </div>
  );
}
