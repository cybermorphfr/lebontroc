"use client";

import { useState, type InputHTMLAttributes, type ReactNode } from "react";

import { Input } from "./Input";

/**
 * Champ mot de passe avec bascule voir/masquer — partout où l'on saisit un
 * mot de passe (connexion, inscription, suppression de compte).
 */
export function PasswordInput({
  id,
  label,
  hint,
  hintValid,
  error,
  ...rest
}: InputHTMLAttributes<HTMLInputElement> & {
  id: string;
  label?: string;
  hint?: ReactNode;
  hintValid?: boolean;
  error?: string;
}) {
  const [visible, setVisible] = useState(false);
  return (
    <div className="relative">
      <Input
        id={id}
        type={visible ? "text" : "password"}
        label={label}
        hint={hint}
        hintValid={hintValid}
        error={error}
        className="pr-10"
        {...rest}
      />
      <button
        type="button"
        aria-label={visible ? "Masquer le mot de passe" : "Afficher le mot de passe"}
        aria-pressed={visible}
        onClick={() => setVisible((v) => !v)}
        className={`absolute right-3 cursor-pointer text-neutre-500 transition-colors hover:text-encre ${
          label ? "top-7" : "top-2"
        }`}
      >
        <EyeIcon open={visible} />
      </button>
    </div>
  );
}

function EyeIcon({ open }: { open: boolean }) {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      {open ? (
        <>
          <path d="M9.88 9.88a3 3 0 1 0 4.24 4.24" />
          <path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68" />
          <path d="M6.61 6.61A13.526 13.526 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61" />
          <line x1="2" x2="22" y1="2" y2="22" />
        </>
      ) : (
        <>
          <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" />
          <circle cx="12" cy="12" r="3" />
        </>
      )}
    </svg>
  );
}
