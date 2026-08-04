import type { InputHTMLAttributes, ReactNode } from "react";

type InputProps = InputHTMLAttributes<HTMLInputElement> & {
  label?: string;
  /** Texte d'aide sous le champ. */
  hint?: ReactNode;
  /** Message d'erreur — remplace le hint et passe le champ en terracotta. */
  error?: string;
  /** Le hint passe en sauge (règle satisfaite, ex. mot de passe assez long). */
  hintValid?: boolean;
};

/** Champ pilule Lebontroc avec label, hint et état d'erreur accessibles. */
export function Input({
  label,
  hint,
  error,
  hintValid = false,
  id,
  className = "",
  ...rest
}: InputProps) {
  const describedBy = error ? `${id}-erreur` : hint ? `${id}-hint` : undefined;
  return (
    <div className="flex flex-col">
      {label ? (
        <label htmlFor={id} className="mb-1.5 text-xs text-encre/70">
          {label}
        </label>
      ) : null}
      <input
        id={id}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        className={`min-h-9 w-full rounded-full border bg-sable px-4 py-1.5 text-sm text-encre caret-terracotta-500 transition-colors placeholder:text-neutre-500 hover:border-encre/45 focus-visible:border-terracotta-500 focus-visible:outline-none ${
          error ? "border-terracotta-500" : "border-neutre-300"
        } ${className}`}
        {...rest}
      />
      {error ? (
        <p id={`${id}-erreur`} className="mt-1 text-[11px] text-terracotta-700">
          {error}
        </p>
      ) : hint ? (
        <p
          id={`${id}-hint`}
          className={`mt-1 text-[11px] ${hintValid ? "text-sauge-700" : "text-neutre-700"}`}
        >
          {hint}
        </p>
      ) : null}
    </div>
  );
}
